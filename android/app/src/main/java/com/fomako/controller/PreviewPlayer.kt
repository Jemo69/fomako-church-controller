package com.fomako.controller

import android.graphics.Bitmap
import android.graphics.BitmapFactory
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
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import org.json.JSONObject

fun buildRtspUrl(cam: CameraConfig): String {
    val enc = { s: String -> URLEncoder.encode(s, "UTF-8").replace("+", "%20") }
    return if (cam.username.isBlank()) {
        "rtsp://${cam.ip}:554/1"
    } else {
        "rtsp://${enc(cam.username)}:${enc(cam.password)}@${cam.ip}:554/1"
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
            } else if (useRelaySnap) {
                RelaySnapshotView(state, cam, relay)
            } else {
                DirectSnapshotView(state, cam)
            }
        }

        Text(
            text = if (useRelaySnap) {
                "Relay JPEG preview (~1 frame/sec via $relay)."
            } else {
                "Direct JPEG preview (http://${cam.ip}/action_snapshot, ~1 frame/sec)."
            },
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun DirectSnapshotView(state: ControllerState, cam: CameraConfig) {
    val endpoint = "http://${cam.ip}"
    HttpSnapshotView(
        state = state,
        cam = cam,
        endpoint = endpoint,
        urlFor = { seq -> "$endpoint/action_snapshot?t=$seq" },
    )
}

@Composable
private fun RelaySnapshotView(state: ControllerState, cam: CameraConfig, relay: String) {
    HttpSnapshotView(
        state = state,
        cam = cam,
        endpoint = relay,
        urlFor = { seq ->
            val user = URLEncoder.encode(cam.username, "UTF-8")
            val password = URLEncoder.encode(cam.password, "UTF-8")
            val ip = URLEncoder.encode(cam.ip, "UTF-8")
            "$relay/api/snapshot?ip=$ip&user=$user&pass=$password&seq=$seq"
        },
    )
}

@Composable
private fun HttpSnapshotView(
    state: ControllerState,
    cam: CameraConfig,
    endpoint: String,
    urlFor: (Long) -> String,
) {
    var bitmap by remember(endpoint, cam.ip, cam.username, cam.password) { mutableStateOf<Bitmap?>(null) }

    LaunchedEffect(endpoint, cam.ip, cam.username, cam.password) {
        var frames = 0
        state.previewStatus = "Connecting ${cam.ip}…"
        while (isActive) {
            val result = withContext(Dispatchers.IO) {
                runCatching {
                    val conn = URL(urlFor(System.currentTimeMillis())).openConnection() as HttpURLConnection
                    try {
                        conn.connectTimeout = 3000
                        conn.readTimeout = 12_000
                        val code = conn.responseCode
                        if (code != 200) {
                            val body = conn.errorStream?.bufferedReader()?.readText().orEmpty()
                            val detail = runCatching { JSONObject(body).optString("error", body) }.getOrDefault(body)
                            error(if (detail.isBlank()) "HTTP $code" else detail)
                        }
                        val bytes = conn.inputStream.use { it.readBytes() }
                        BitmapFactory.decodeByteArray(bytes, 0, bytes.size) ?: error("camera returned a bad JPEG")
                    } finally {
                        conn.disconnect()
                    }
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
