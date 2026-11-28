package com.fomako.controller

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

@Composable
fun ControlScreen(state: ControllerState) {
    CameraSwitcherRow(state)

    BoxWithConstraints(modifier = Modifier.fillMaxWidth()) {
        val wide = maxWidth >= 560.dp && (maxWidth >= 700.dp || maxWidth > maxHeight)
        if (wide) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.Top
            ) {
                Column(
                    modifier = Modifier.weight(1.1f),
                    verticalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    LivePreviewCard(state)
                    ZoomFocusPowerCard(state)
                }
                Column(
                    modifier = Modifier.weight(1f),
                    verticalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    PtzPadCard(state)
                    SpeedsPresetsCard(state)
                }
            }
        } else {
            Column(
                modifier = Modifier.fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                LivePreviewCard(state)
                PtzPadCard(state)
                ZoomFocusPowerCard(state)
                SpeedsPresetsCard(state)
            }
        }
    }
}

@Composable
private fun PtzPadCard(state: ControllerState) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            SectionHeading("Pan / Tilt (tap to jog, STOP to halt)")
            val drive = { label: String, code: Int ->
                state.sendVisca(label, "pt_drive", state.panSpeed, state.tiltSpeed, code)
            }
            @Composable
            fun PtzKey(symbol: String, a11y: String, modifier: Modifier, onClick: () -> Unit) {
                OutlinedButton(
                    onClick = onClick,
                    modifier = modifier
                        .height(52.dp)
                        .semantics { contentDescription = a11y },
                    contentPadding = PaddingValues(horizontal = 4.dp, vertical = 8.dp)
                ) {
                    Text(symbol, fontWeight = FontWeight.SemiBold)
                }
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                PtzKey("↖", "Pan Up-Left", Modifier.weight(1f)) { drive("Up-Left", 4) }
                PtzKey("↑", "Tilt Up", Modifier.weight(1f)) { drive("Up", 0) }
                PtzKey("↗", "Pan Up-Right", Modifier.weight(1f)) { drive("Up-Right", 5) }
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                PtzKey("←", "Pan Left", Modifier.weight(1f)) { drive("Left", 2) }
                PtzKey("HOME", "Return camera to Home", Modifier.weight(1f)) { drive("Home", 8) }
                PtzKey("→", "Pan Right", Modifier.weight(1f)) { drive("Right", 3) }
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                PtzKey("↙", "Pan Down-Left", Modifier.weight(1f)) { drive("Down-Left", 6) }
                PtzKey("↓", "Tilt Down", Modifier.weight(1f)) { drive("Down", 1) }
                PtzKey("↘", "Pan Down-Right", Modifier.weight(1f)) { drive("Down-Right", 7) }
            }
            Button(
                onClick = { drive("Stop", 9) },
                modifier = Modifier
                    .fillMaxWidth()
                    .height(48.dp)
                    .semantics { contentDescription = "Stop pan and tilt" },
                shape = RoundedCornerShape(10.dp),
                colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary)
            ) {
                Text("STOP", fontWeight = FontWeight.Bold)
            }
        }
    }
}

@Composable
private fun ZoomFocusPowerCard(state: ControllerState) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            SectionHeading("Zoom")
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(
                    onClick = { state.sendVisca("Zoom Out", "zoom_wide", state.zoomSpeed) },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Zoom wide out" },
                    contentPadding = PaddingValues(horizontal = 6.dp)
                ) { Text("Wide −") }
                OutlinedButton(
                    onClick = { state.sendVisca("Zoom Stop", "zoom_stop") },
                    modifier = Modifier.weight(0.8f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Stop zoom" },
                    contentPadding = PaddingValues(horizontal = 6.dp)
                ) { Text("Stop") }
                OutlinedButton(
                    onClick = { state.sendVisca("Zoom In", "zoom_tele", state.zoomSpeed) },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Zoom telephoto in" },
                    contentPadding = PaddingValues(horizontal = 6.dp)
                ) { Text("Tele +") }
            }

            SectionHeading("Focus")
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(
                    onClick = { state.sendVisca("Focus Near", "focus_near") },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Focus near" },
                    contentPadding = PaddingValues(horizontal = 6.dp)
                ) { Text("Near") }
                OutlinedButton(
                    onClick = { state.sendVisca("Focus Stop", "focus_stop") },
                    modifier = Modifier.weight(0.8f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Stop focus" },
                    contentPadding = PaddingValues(horizontal = 6.dp)
                ) { Text("Stop") }
                OutlinedButton(
                    onClick = { state.sendVisca("Focus Far", "focus_far") },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Focus far" },
                    contentPadding = PaddingValues(horizontal = 6.dp)
                ) { Text("Far") }
            }
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(
                    onClick = { state.sendVisca("Focus Auto", "focus_auto") },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Auto focus" },
                    contentPadding = PaddingValues(horizontal = 4.dp)
                ) { Text("Auto") }
                OutlinedButton(
                    onClick = { state.sendVisca("Focus Manual", "focus_manual") },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Manual focus" },
                    contentPadding = PaddingValues(horizontal = 4.dp)
                ) { Text("Manual") }
                OutlinedButton(
                    onClick = { state.sendVisca("Focus 1-Push", "focus_one_push") },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "One-push focus" },
                    contentPadding = PaddingValues(horizontal = 4.dp)
                ) { Text("1-Push") }
            }

            SectionHeading("Power")
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(
                    onClick = { state.sendVisca("Power ON", "power", 1) },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                ) { Text("Power ON") }
                OutlinedButton(
                    onClick = { state.sendVisca("Standby", "power", 0) },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                ) { Text("Standby") }
            }

            // Camera-side AI tracking (AI-tracking models only). While it runs the
            // camera ignores pan/tilt; switch target with F4 on the IR remote.
            SectionHeading("AI auto tracking")
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(
                    onClick = { state.sendVisca("AI Tracking ON", "tracking", 1, 0) },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Turn AI tracking on" }
                ) { Text("Track ON") }
                OutlinedButton(
                    onClick = { state.sendVisca("AI Area Tracking ON", "tracking", 1, 1) },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Turn AI area tracking on" }
                ) { Text("Area") }
                OutlinedButton(
                    onClick = { state.sendVisca("AI Tracking OFF", "tracking", 0, 0) },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Turn AI tracking off" }
                ) { Text("OFF") }
            }
            Text(
                "Real-time follows one person (F4 switches target). Area needs areas saved in the camera web page. PTZ is locked while tracking runs.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

@Composable
private fun SpeedsPresetsCard(state: ControllerState) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            SectionHeading("Speeds")
            StepperRow(
                label = "Pan speed",
                value = "${state.panSpeed} (1-24)",
                onMinus = { state.panSpeed = (state.panSpeed - 1).coerceAtLeast(1) },
                onPlus = { state.panSpeed = (state.panSpeed + 1).coerceAtMost(24) }
            )
            StepperRow(
                label = "Tilt speed",
                value = "${state.tiltSpeed} (1-20)",
                onMinus = { state.tiltSpeed = (state.tiltSpeed - 1).coerceAtLeast(1) },
                onPlus = { state.tiltSpeed = (state.tiltSpeed + 1).coerceAtMost(20) }
            )
            StepperRow(
                label = "Zoom speed",
                value = "${state.zoomSpeed} (0-7)",
                onMinus = { state.zoomSpeed = (state.zoomSpeed - 1).coerceAtLeast(0) },
                onPlus = { state.zoomSpeed = (state.zoomSpeed + 1).coerceAtMost(7) }
            )

            SectionHeading("Presets (0-254, remote 0-9)")
            StepperRow(
                label = "Preset No.",
                value = "${state.presetSel}",
                onMinus = { state.presetSel = (state.presetSel - 1).coerceAtLeast(0) },
                onPlus = { state.presetSel = (state.presetSel + 1).coerceAtMost(254) }
            )
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(
                    onClick = { state.sendVisca("Preset ${state.presetSel} Recall", "preset_recall", state.presetSel) },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Recall selected preset" },
                    contentPadding = PaddingValues(horizontal = 4.dp)
                ) { Text("Recall") }
                Button(
                    onClick = { state.sendVisca("Preset ${state.presetSel} Set", "preset_set", state.presetSel) },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Save current position to selected preset" },
                    contentPadding = PaddingValues(horizontal = 4.dp)
                ) { Text("Set") }
                OutlinedButton(
                    onClick = { state.sendVisca("Preset ${state.presetSel} Clear", "preset_clear", state.presetSel) },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                        .semantics { contentDescription = "Clear selected preset" },
                    contentPadding = PaddingValues(horizontal = 4.dp)
                ) { Text("Clear") }
            }

            Text(
                "Quick recall 0-9:",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            for (r in 0..1) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    for (c in 0..4) {
                        val n = r * 5 + c
                        OutlinedButton(
                            onClick = { state.sendVisca("Preset $n Recall", "preset_recall", n) },
                            modifier = Modifier
                                .weight(1f)
                                .defaultMinSize(minHeight = 48.dp)
                                .semantics { contentDescription = "Recall preset $n" },
                            contentPadding = PaddingValues(0.dp)
                        ) {
                            Text("$n", fontWeight = FontWeight.Medium)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun CameraSwitcherRow(state: ControllerState) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text(
            text = "Camera:",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        state.cameras.forEachIndexed { idx, cam ->
            val on = idx == state.selected
            FilterChip(
                selected = on,
                onClick = {
                    state.selected = idx
                    state.setStatusMsg("Selected ${cam.name}")
                },
                modifier = Modifier.semantics {
                    contentDescription = "Camera ${cam.name}, ${cam.ip}"
                },
                label = { Text(if (on) "● ${cam.name}" else cam.name) }
            )
        }
    }
}
