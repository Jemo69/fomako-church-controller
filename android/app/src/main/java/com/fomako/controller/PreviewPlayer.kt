package com.fomako.controller

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.MediaPlayer
import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import java.util.Locale
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import org.json.JSONObject

/// RTSP URLs for the preview, sub stream first: FoMaKo firmware exposes the
/// sub stream as `/2` or `/live/av1`, the main as `/1` or `/live/av0`.
private val PREVIEW_PATHS = listOf("2", "live/av1", "1", "live/av0")

fun buildRtspUrl(cam: CameraConfig, path: String = "1"): String {
    val enc = { s: String -> URLEncoder.encode(s, "UTF-8").replace("+", "%20") }
    return if (cam.username.isBlank()) {
        "rtsp://${cam.ip}:554/$path"
    } else {
        "rtsp://${enc(cam.username)}:${enc(cam.password)}@${cam.ip}:554/$path"
    }
}

@Composable
fun LivePreviewCard(state: ControllerState) {
    val cam = state.current()
    val relay = state.relayUrl.trim().trimEnd('/')
    val useRelay = state.useRelay && relay.isNotEmpty()

    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                text = "Live preview",
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontWeight = FontWeight.SemiBold,
            )
            Text(
                text = state.previewStatus,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
                maxLines = 1,
            )
            OutlinedButton(onClick = {
                state.previewOn = !state.previewOn
                state.previewStatus = if (state.previewOn) "Preview starting…" else "Preview paused"
            }) {
                Text(if (state.previewOn) "Pause" else "Resume")
            }
        }

        val screenH = LocalConfiguration.current.screenHeightDp
        val previewMaxH = (screenH * 0.34f).dp.coerceIn(130.dp, 280.dp)
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .widthIn(max = 520.dp)
                .heightIn(max = previewMaxH)
                .aspectRatio(16f / 9f)
                .clip(RoundedCornerShape(10.dp))
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(10.dp)),
            contentAlignment = Alignment.Center
        ) {
            if (!state.previewOn) {
                Text(
                    "Preview paused",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    style = MaterialTheme.typography.bodyMedium
                )
            } else if (useRelay) {
                RelayStreamView(state, cam, relay)
            } else {
                DirectRtspView(state, cam)
            }
        }

        Text(
            text = if (useRelay) {
                "Live MJPEG stream from the relay ($relay), ~30 fps."
            } else {
                "Live RTSP video from ${cam.ip}, ~30 fps."
            },
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/// Direct RTSP through Android's MediaPlayer: full frame rate, no relay.
@Composable
private fun DirectRtspView(state: ControllerState, cam: CameraConfig) {
    var attempt by remember(cam.ip, cam.username, cam.password) { mutableStateOf(0) }
    val path = PREVIEW_PATHS[attempt.coerceIn(0, PREVIEW_PATHS.lastIndex)]
    var player by remember { mutableStateOf<MediaPlayer?>(null) }

    LaunchedEffect(cam.ip, cam.username, cam.password, attempt) {
        if (attempt == 0) state.previewStatus = "Connecting ${cam.ip}…"
    }

    key(cam.ip, cam.username, cam.password, attempt) {
        AndroidView(
            modifier = Modifier.fillMaxSize(),
            factory = { ctx ->
                SurfaceView(ctx).apply {
                    holder.addCallback(object : SurfaceHolder.Callback {
                        override fun surfaceCreated(holder: SurfaceHolder) {
                            runCatching {
                                MediaPlayer().apply {
                                    setDataSource(buildRtspUrl(cam, path))
                                    setSurface(holder.surface)
                                    setOnPreparedListener {
                                        it.start()
                                        state.previewStatus = "live · RTSP $path"
                                    }
                                    setOnInfoListener { _, what, _ ->
                                        if (what == MediaPlayer.MEDIA_INFO_BUFFERING_START) {
                                            state.previewStatus = "buffering · RTSP $path"
                                        }
                                        false
                                    }
                                    setOnErrorListener { _, what, extra ->
                                        state.previewStatus = "RTSP error ($what/$extra) — retrying…"
                                        postDelayed({
                                            if (isAttachedToWindow) attempt = (attempt + 1) % PREVIEW_PATHS.size
                                        }, 2500)
                                        true
                                    }
                                    prepareAsync()
                                }.also { player = it }
                            }.onFailure { e ->
                                state.previewStatus = "RTSP failed: ${e.message ?: "unknown"} — retrying…"
                                postDelayed({
                                    if (isAttachedToWindow) attempt = (attempt + 1) % PREVIEW_PATHS.size
                                }, 2500)
                            }
                        }

                        override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {}

                        override fun surfaceDestroyed(holder: SurfaceHolder) {
                            runCatching { player?.release() }
                            player = null
                        }
                    })
                }
            },
            onRelease = {
                runCatching { player?.release() }
                player = null
            },
        )
    }
}

/// Relay `/api/stream`: multipart JPEG at the relay's decode rate (~30 fps).
@Composable
private fun RelayStreamView(state: ControllerState, cam: CameraConfig, relay: String) {
    var bitmap by remember(relay, cam.ip, cam.username, cam.password) { mutableStateOf<Bitmap?>(null) }

    LaunchedEffect(relay, cam.ip, cam.username, cam.password) {
        val ip = URLEncoder.encode(cam.ip, "UTF-8")
        val user = URLEncoder.encode(cam.username, "UTF-8")
        val password = URLEncoder.encode(cam.password, "UTF-8")
        val url = "$relay/api/stream?ip=$ip&user=$user&pass=$password"
        state.previewStatus = "Connecting ${cam.ip}…"
        withContext(Dispatchers.IO) {
            while (isActive) {
                try {
                    val conn = URL(url).openConnection() as HttpURLConnection
                    conn.connectTimeout = 3000
                    conn.readTimeout = 15_000
                    try {
                        val code = conn.responseCode
                        if (code != 200) {
                            val body = conn.errorStream?.bufferedReader()?.readText().orEmpty()
                            val detail = runCatching { JSONObject(body).optString("error", body) }.getOrDefault(body)
                            error(if (detail.isBlank()) "HTTP $code" else detail)
                        }
                        val buf = ByteArrayOutputStream()
                        val chunk = ByteArray(64 * 1024)
                        var windowStart = System.nanoTime()
                        var windowFrames = 0
                        conn.inputStream.use { input ->
                            while (isActive) {
                                val n = input.read(chunk)
                                if (n < 0) break
                                buf.write(chunk, 0, n)
                                var bytes = buf.toByteArray()
                                buf.reset()
                                while (isActive) {
                                    val start = jpegStart(bytes)
                                    if (start < 0) {
                                        val keep = if (bytes.size > 2) bytes.copyOfRange(bytes.size - 2, bytes.size) else bytes
                                        buf.write(keep)
                                        break
                                    }
                                    bytes = bytes.copyOfRange(start, bytes.size)
                                    val end = jpegEnd(bytes)
                                    if (end < 0) {
                                        if (buf.size() + bytes.size > 16 * 1024 * 1024) buf.reset() else buf.write(bytes)
                                        break
                                    }
                                    val jpeg = bytes.copyOfRange(0, end)
                                    bytes = bytes.copyOfRange(end, bytes.size)
                                    BitmapFactory.decodeByteArray(jpeg, 0, jpeg.size)?.let { decoded ->
                                        withContext(Dispatchers.Main) { bitmap = decoded }
                                        windowFrames++
                                        val now = System.nanoTime()
                                        if (now - windowStart >= 1_000_000_000L) {
                                            val fps = windowFrames * 1e9 / (now - windowStart)
                                            val status = String.format(Locale.US, "live · %.0f fps", fps)
                                            withContext(Dispatchers.Main) { state.previewStatus = status }
                                            windowStart = now
                                            windowFrames = 0
                                        }
                                    }
                                    if (bytes.isEmpty()) break
                                }
                                if (buf.size() > 16 * 1024 * 1024) buf.reset()
                            }
                        }
                    } finally {
                        conn.disconnect()
                    }
                } catch (e: Exception) {
                    if (isActive) {
                        withContext(Dispatchers.Main) {
                            state.previewStatus = "retrying… ${e.message ?: "relay offline"}"
                        }
                        delay(3000)
                    }
                }
            }
        }
    }

    val bmp = bitmap
    if (bmp != null) {
        Image(
            bitmap = bmp.asImageBitmap(),
            contentDescription = "Camera preview",
            contentScale = ContentScale.Crop,
            modifier = Modifier.fillMaxSize()
        )
    } else {
        Text(
            text = state.previewStatus,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(12.dp)
        )
    }
}

private fun jpegStart(data: ByteArray): Int {
    for (i in 0..data.size - 3) {
        if (data[i] == 0xFF.toByte() && data[i + 1] == 0xD8.toByte() && data[i + 2] == 0xFF.toByte()) return i
    }
    return -1
}

private fun jpegEnd(data: ByteArray): Int {
    for (i in 2..data.size - 2) {
        if (data[i] == 0xFF.toByte() && data[i + 1] == 0xD9.toByte()) return i + 2
    }
    return -1
}
