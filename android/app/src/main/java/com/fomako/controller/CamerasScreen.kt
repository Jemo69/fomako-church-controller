package com.fomako.controller

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun CamerasScreen(state: ControllerState) {
    var nameInput by remember { mutableStateOf("New Camera") }
    var ipInput by remember { mutableStateOf("192.168.5.163") }
    var userInput by remember { mutableStateOf("") }
    var passInput by remember { mutableStateOf("") }
    var credUser by remember { mutableStateOf("") }
    var credPass by remember { mutableStateOf("") }

    Text(
        text = "Cameras — switch, scan, add, remove",
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.SemiBold,
    )
    Text(
        text = "Connects directly to cameras over WiFi (UDP :52381 / TCP :5678) — no PC needed.",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )

    // Saved cameras
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            SectionHeading("Saved (${state.cameras.size}). Tap Select to switch.")
            state.cameras.forEachIndexed { idx, cam ->
                val on = idx == state.selected
                Column(
                    modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    Text(
                        text = "${if (on) "●" else "○"} #${cam.address}  ${cam.name}  ${cam.ip}:${cam.protocol.defaultPort}",
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = if (on) FontWeight.SemiBold else FontWeight.Normal,
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedButton(
                            onClick = {
                                state.selected = idx
                                state.setStatusMsg("Selected ${cam.name}")
                            },
                            modifier = Modifier
                                .defaultMinSize(minHeight = 48.dp)
                                .semantics { contentDescription = "Select ${cam.name}" }
                        ) { Text("Select") }
                        OutlinedButton(
                            onClick = { state.testCamera(idx) },
                            modifier = Modifier
                                .defaultMinSize(minHeight = 48.dp)
                                .semantics { contentDescription = "Test ${cam.name}" }
                        ) { Text("Test") }
                    }
                }
            }
            if (state.cameras.size > 1) {
                OutlinedButton(onClick = {
                    val idx = state.selected.coerceIn(0, state.cameras.lastIndex)
                    state.cameras.removeAt(idx)
                    state.selected = 0
                    state.saveCameras()
                    state.setStatusMsg("Camera removed")
                }) { Text("Remove selected") }
            }
        }
    }

    // Login for selected camera
    val current = state.current()
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            SectionHeading("Login for ${current.name} (RTSP/web; VISCA needs none)")
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = credUser,
                    onValueChange = { credUser = it },
                    label = { Text("User (${current.username})") },
                    singleLine = true,
                    modifier = Modifier.weight(1f)
                )
                OutlinedTextField(
                    value = credPass,
                    onValueChange = { credPass = it },
                    label = { Text("Password") },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    modifier = Modifier.weight(1f)
                )
            }
            OutlinedButton(onClick = {
                val idx = state.selected.coerceIn(0, state.cameras.lastIndex)
                val old = state.cameras[idx]
                state.cameras[idx] = old.copy(
                    username = credUser.trim().ifEmpty { old.username },
                    password = credPass.trim().ifEmpty { old.password },
                )
                credUser = ""
                credPass = ""
                state.saveCameras()
                state.setStatusMsg("Login saved for ${state.current().name}")
            }) { Text("Save login") }
        }
    }

    // Auto-find LAN scan
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            SectionHeading("Find cameras on WiFi (auto)")
            FlowRow(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                itemVerticalAlignment = Alignment.CenterVertically,
            ) {
                OutlinedTextField(
                    value = state.scanPrefix,
                    onValueChange = { state.scanPrefix = it },
                    label = { Text("Subnet (e.g. 192.168.1)") },
                    singleLine = true,
                    modifier = Modifier.widthIn(min = 150.dp).weight(1f)
                )
                Button(
                    enabled = !state.scanning,
                    onClick = { state.startScan() },
                    modifier = Modifier.defaultMinSize(minHeight = 48.dp)
                ) {
                    Text(if (state.scanning) "Scanning…" else "Scan WiFi")
                }
            }
            Text(
                text = state.scanMessage,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            for (hit in state.scanResults) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Text(
                        text = "${hit.ip} — ${hit.detail}",
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.weight(1f)
                    )
                    OutlinedButton(onClick = {
                        if (state.cameras.any { it.ip == hit.ip }) {
                            state.setStatusMsg("${hit.ip} is already saved")
                        } else {
                            val proto = if (hit.udp52381) ProtoMode.SonyUdp else ProtoMode.Tcp
                            val addr = (state.cameras.size + 1).coerceAtMost(7)
                            state.cameras.add(
                                CameraConfig(
                                    name = "Camera ${hit.ip}",
                                    ip = hit.ip,
                                    protocol = proto,
                                    address = addr,
                                )
                            )
                            state.selected = state.cameras.lastIndex
                            state.saveCameras()
                            state.setStatusMsg("Added ${hit.ip}")
                        }
                    }) { Text("Add") }
                }
            }
        }
    }

    // Add camera form
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            SectionHeading("Add camera manually")
            OutlinedTextField(
                value = nameInput,
                onValueChange = { nameInput = it },
                label = { Text("Name") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth()
            )
            OutlinedTextField(
                value = ipInput,
                onValueChange = { ipInput = it },
                label = { Text("IP Address") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth()
            )
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = userInput,
                    onValueChange = { userInput = it },
                    label = { Text("User (admin)") },
                    singleLine = true,
                    modifier = Modifier.weight(1f)
                )
                OutlinedTextField(
                    value = passInput,
                    onValueChange = { passInput = it },
                    label = { Text("Pass (admin)") },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    modifier = Modifier.weight(1f)
                )
            }
            FlowRow(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Button(
                    onClick = {
                        val ip = ipInput.trim()
                        if (ip.isEmpty()) {
                            state.setStatusMsg("Enter an IP first")
                        } else {
                            state.cameras.add(
                                CameraConfig(
                                    name = nameInput.trim().ifEmpty { "Camera" },
                                    ip = ip,
                                    protocol = ProtoMode.SonyUdp,
                                    address = (state.cameras.size + 1).coerceAtMost(7),
                                    username = userInput.trim().ifEmpty { "admin" },
                                    password = passInput.trim().ifEmpty { "admin" },
                                )
                            )
                            state.selected = state.cameras.lastIndex
                            state.saveCameras()
                            state.setStatusMsg("Added ${state.current().name}")
                        }
                    },
                    modifier = Modifier.widthIn(min = 140.dp).weight(1f).defaultMinSize(minHeight = 48.dp)
                ) { Text("Add Sony UDP :52381") }
                OutlinedButton(
                    onClick = {
                        val ip = ipInput.trim()
                        if (ip.isEmpty()) {
                            state.setStatusMsg("Enter an IP first")
                        } else {
                            state.cameras.add(
                                CameraConfig(
                                    name = nameInput.trim().ifEmpty { "Camera" },
                                    ip = ip,
                                    protocol = ProtoMode.Tcp,
                                    address = (state.cameras.size + 1).coerceAtMost(7),
                                    username = userInput.trim().ifEmpty { "admin" },
                                    password = passInput.trim().ifEmpty { "admin" },
                                )
                            )
                            state.selected = state.cameras.lastIndex
                            state.saveCameras()
                            state.setStatusMsg("Added ${state.current().name}")
                        }
                    },
                    modifier = Modifier.widthIn(min = 130.dp).weight(1f).defaultMinSize(minHeight = 48.dp)
                ) { Text("Add TCP :5678") }
            }
        }
    }

    // Optional relay (for off-subnet preview or bridge)
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            SectionHeading("Optional LAN relay (only if needed)")
            Text(
                text = "Leave OFF on church WiFi — Android talks directly to cameras. Turn ON only if using scripts/visca-relay.py for snapshot preview.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                OutlinedButton(onClick = {
                    state.useRelay = !state.useRelay
                    state.setStatusMsg(if (state.useRelay) "Relay ON" else "Relay OFF — direct sockets")
                }) {
                    Text(if (state.useRelay) "Relay: ON" else "Relay: OFF")
                }
                OutlinedTextField(
                    value = state.relayUrl,
                    onValueChange = { state.relayUrl = it },
                    label = { Text("http://192.168.1.10:8080") },
                    singleLine = true,
                    modifier = Modifier.weight(1f)
                )
            }
        }
    }

    // Streams
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            SectionHeading("Streams (${current.ip})")
            Text("Main: rtsp://${current.ip}:554/live/av0", fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
            Text("Sub:  rtsp://${current.ip}:554/live/av1", fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
            Text("RTMP: rtmp://${current.ip}:1935/live/av0", fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
            Text("Web:  http://${current.ip}", fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
        }
    }
}
