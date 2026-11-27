package com.fomako.controller

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.MediaPlayer
import android.net.Uri
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
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder

fun buildRtspUrl(cam: CameraConfig): String {
    val enc = { s: String -> URLEncoder.encode(s, "UTF-8").replace("+", "%20") }
    return if (cam.username.isBlank()) {
        "rtsp://${cam.ip}:554/live/av0"
    } else {
        "rtsp://${enc(cam.username)}:${enc(cam.password)}@${cam.ip}:554/live/av0"
    }
}

@Composable
fun LivePreviewCard(state: ControllerState) {
    val cam = state.current()
    val relay = state.relayUrl.trim().trimEnd('/')
    val useRelaySnap = state.useRelay && relay.isNotEmpty()

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
            OutlinedButton(onClick = { state.previewOn = !state.previewOn }) {
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
                state.previewStatus = "Preview paused"
                Text(
                    "Preview paused",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    style = MaterialTheme.typography.bodyMedium
                )
            } else if (useRelaySnap) {
                RelaySnapshotView(state, cam, relay)
            } else {
                DirectRtspView(state, cam)
            }
        }

        Text(
            text = if (useRelaySnap) {
                "Relay JPEG preview (~1 frame/sec via $relay)."
            } else {
                "Direct RTSP preview (rtsp://${cam.ip}:554/live/av0). Check camera login on Cameras tab if auth fails."
            },
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun DirectRtspView(state: ControllerState, cam: CameraConfig) {
    val context = LocalContext.current
    val rtspUrl = remember(cam.ip, cam.username, cam.password) { buildRtspUrl(cam) }
    var surfaceReady by remember { mutableStateOf<SurfaceHolder?>(null) }

    DisposableEffect(rtspUrl, surfaceReady) {
        val holder = surfaceReady
        if (holder == null) {
            onDispose { }
        } else {
            state.previewStatus = "Connecting RTSP ${cam.ip}…"
            val player = MediaPlayer()
            var released = false
            runCatching {
                player.setDisplay(holder)
                player.setDataSource(context, Uri.parse(rtspUrl))
                player.setOnPreparedListener { mp ->
                    if (!released) {
                        state.previewStatus = "live RTSP · ${cam.ip}"
                        mp.start()
                    }
                }
                player.setOnErrorListener { _, what, extra ->
                    if (!released) {
                        state.previewStatus = "RTSP error ($what/$extra) — check WiFi/login"
                    }
                    true
                }
                player.prepareAsync()
            }.onFailure { e ->
                state.previewStatus = "RTSP: ${e.message ?: "failed"}"
            }
            onDispose {
                released = true
                runCatching { player.stop() }
                runCatching { player.release() }
            }
        }
    }

    AndroidView(
        factory = { ctx ->
            SurfaceView(ctx).apply {
                holder.addCallback(object : SurfaceHolder.Callback {
                    override fun surfaceCreated(h: SurfaceHolder) {
                        surfaceReady = h
                    }
                    override fun surfaceChanged(h: SurfaceHolder, f: Int, w: Int, ht: Int) {}
                    override fun surfaceDestroyed(h: SurfaceHolder) {
                        if (surfaceReady == h) surfaceReady = null
                    }
                })
            }
        },
        modifier = Modifier.fillMaxSize()
    )
}

@Composable
private fun RelaySnapshotView(state: ControllerState, cam: CameraConfig, relay: String) {
    var bitmap by remember { mutableStateOf<Bitmap?>(null) }

    LaunchedEffect(cam.ip, cam.username, cam.password, relay) {
        var frames = 0
        state.previewStatus = "Connecting relay ${cam.ip}…"
        while (isActive) {
            val result = withContext(Dispatchers.IO) {
                runCatching {
                    val u = URLEncoder.encode(cam.username, "UTF-8")
                    val p = URLEncoder.encode(cam.password, "UTF-8")
                    val conn = (URL("$relay/api/snapshot?ip=${cam.ip}&user=$u&pass=$p").openConnection() as HttpURLConnection).apply {
                        connectTimeout = 3000
                        readTimeout = 8000
                    }
                    val code = conn.responseCode
                    if (code != 200) error("HTTP $code")
                    val bytes = conn.inputStream.use { it.readBytes() }
                    BitmapFactory.decodeByteArray(bytes, 0, bytes.size) ?: error("bad jpeg")
                }
            }
            if (result.isSuccess) {
                frames++
                bitmap = result.getOrNull()
                state.previewStatus = "live · frame $frames"
                delay(1500)
            } else {
                state.previewStatus = "retrying… ${result.exceptionOrNull()?.message ?: "offline"}"
                delay(3500)
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
