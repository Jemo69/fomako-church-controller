package com.fomako.controller

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

@Composable
fun ImageScreen(state: ControllerState) {
    val cam = state.current()
    Text(
        text = "Image settings -> ${cam.name} @ ${cam.ip}",
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.SemiBold,
    )
    Text(
        text = "Each row sends real VISCA to the camera (§5.2).",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )

    // Exposure
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            SectionHeading("Exposure (${state.aeLabel()})")
            val aeModes = listOf("Auto", "Manual", "Shutter", "Iris", "Bright")
            Row(
                modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                aeModes.forEachIndexed { idx, name ->
                    FilterChip(
                        selected = state.aeMode == idx,
                        onClick = {
                            state.aeMode = idx
                            state.sendVisca("Exposure ${state.aeLabel()}", "ae_mode", state.aeByte())
                        },
                        label = { Text(name) }
                    )
                }
            }
            StepperRow("Shutter", "0x%02X".format(state.shutter),
                { state.shutter = (state.shutter - 1).coerceAtLeast(0) },
                { state.shutter = (state.shutter + 1).coerceAtMost(20) })
            ApplyRow("Shutter direct", "VISCA 4A") { state.sendVisca("Shutter", "shutter", state.shutter) }

            StepperRow("Iris", "0x%02X".format(state.iris),
                { state.iris = (state.iris - 1).coerceAtLeast(0) },
                { state.iris = (state.iris + 1).coerceAtMost(20) })
            ApplyRow("Iris direct", "VISCA 4B") { state.sendVisca("Iris", "iris", state.iris) }

            StepperRow("Gain limit", "${state.gainLimit}",
                { state.gainLimit = (state.gainLimit - 1).coerceAtLeast(0) },
                { state.gainLimit = (state.gainLimit + 1).coerceAtMost(15) })
            ApplyRow("Gain limit", "VISCA 2C") { state.sendVisca("Gain Limit", "gain_limit", state.gainLimit) }

            StepperRow("Bright level", "${state.brightLvl}",
                { state.brightLvl = (state.brightLvl - 1).coerceAtLeast(0) },
                { state.brightLvl = (state.brightLvl + 1).coerceAtMost(23) })
            ApplyRow("Bright", "VISCA 4D") { state.sendVisca("Bright Level", "bright", state.brightLvl) }
        }
    }

    // Backlight / WDR / NR
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            SectionHeading("Backlight / WDR / NR")
            ApplyRow("Backlight", if (state.backlight) "ON" else "OFF") {
                state.backlight = !state.backlight
                state.sendVisca("Backlight", "backlight", if (state.backlight) 1 else 0)
            }
            StepperRow("WDR level", "${state.wdr}",
                { state.wdr = (state.wdr - 1).coerceAtLeast(0) },
                { state.wdr = (state.wdr + 1).coerceAtMost(7) })
            ApplyRow("WDR", "VISCA 51") { state.sendVisca("WDR", "wdr", state.wdr) }

            StepperRow("NR 2D (0-7)", "${state.nr2d}",
                { state.nr2d = (state.nr2d - 1).coerceAtLeast(0) },
                { state.nr2d = (state.nr2d + 1).coerceAtMost(7) })
            StepperRow("NR 3D (0-8)", "${state.nr3d}",
                { state.nr3d = (state.nr3d - 1).coerceAtLeast(0) },
                { state.nr3d = (state.nr3d + 1).coerceAtMost(8) })
            ApplyRow("Noise reduction", "VISCA 53/54") {
                state.sendVisca("NR 2D", "nr2d", state.nr2d)
                state.sendVisca("NR 3D", "nr3d", state.nr3d)
            }
        }
    }

    // White balance / color
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            SectionHeading("White balance / color (${state.wbLabel()})")
            val wbLabels = listOf("Auto" to 0, "3000K" to 1, "4000K" to 2, "1-Push" to 3, "Manual" to 5, "6500K" to 6)
            Row(
                modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                for ((label, code) in wbLabels) {
                    FilterChip(
                        selected = state.wbMode == code,
                        onClick = {
                            state.wbMode = code
                            state.sendVisca("WB ${state.wbLabel()}", "wb_mode", code)
                        },
                        label = { Text(label) }
                    )
                }
            }
            StepperRow("R Gain", "${state.rGain}",
                { state.rGain = (state.rGain - 1).coerceAtLeast(0) },
                { state.rGain = (state.rGain + 1).coerceAtMost(255) })
            StepperRow("B Gain", "${state.bGain}",
                { state.bGain = (state.bGain - 1).coerceAtLeast(0) },
                { state.bGain = (state.bGain + 1).coerceAtMost(255) })
            ApplyRow("R/B Gain", "VISCA 43/44") {
                state.sendVisca("R Gain", "rgain", state.rGain)
                state.sendVisca("B Gain", "bgain", state.bGain)
            }
        }
    }

    // Picture & Misc
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            SectionHeading("Picture & Misc")
            StepperRow("Brightness", "${state.brightness}",
                { state.brightness = (state.brightness - 1).coerceAtLeast(0) },
                { state.brightness = (state.brightness + 1).coerceAtMost(14) })
            ApplyRow("Brightness", "VISCA A1") { state.sendVisca("Brightness", "brightness", state.brightness) }

            StepperRow("Contrast", "${state.contrast}",
                { state.contrast = (state.contrast - 1).coerceAtLeast(0) },
                { state.contrast = (state.contrast + 1).coerceAtMost(14) })
            ApplyRow("Contrast", "VISCA A2") { state.sendVisca("Contrast", "contrast", state.contrast) }

            StepperRow("Saturation 0-14", "${state.saturation}",
                { state.saturation = (state.saturation - 1).coerceAtLeast(0) },
                { state.saturation = (state.saturation + 1).coerceAtMost(14) })
            ApplyRow("Saturation", "VISCA 49") { state.sendVisca("Saturation", "saturation", state.saturation) }

            StepperRow("Sharpness 0-15", "${state.aperture}",
                { state.aperture = (state.aperture - 1).coerceAtLeast(0) },
                { state.aperture = (state.aperture + 1).coerceAtMost(15) })
            ApplyRow("Sharpness", "VISCA 42") { state.sendVisca("Sharpness", "aperture", state.aperture) }

            StepperRow("Gamma 0-4", "${state.gamma}",
                { state.gamma = (state.gamma - 1).coerceAtLeast(0) },
                { state.gamma = (state.gamma + 1).coerceAtMost(4) })
            ApplyRow("Gamma", "VISCA 5B") { state.sendVisca("Gamma", "gamma", state.gamma) }

            ApplyRow("Flicker", when (state.flicker) { 1 -> "50Hz"; 2 -> "60Hz"; else -> "OFF" }) {
                state.flicker = (state.flicker + 1) % 3
                state.sendVisca("Flicker", "flicker", state.flicker)
            }
            ApplyRow("Low-light", if (state.lowlight) "ON" else "OFF") {
                state.lowlight = !state.lowlight
                state.sendVisca("Low-Light", "lowlight", if (state.lowlight) 1 else 0)
            }
            ApplyRow("Flip", when (state.flip) { 1 -> "H"; 2 -> "V"; 3 -> "HV"; else -> "OFF" }) {
                state.flip = (state.flip + 1) % 4
                state.sendVisca("Flip", "flip", state.flip)
            }
            ApplyRow("B/W mode", if (state.bw) "B&W" else "Color") {
                state.bw = !state.bw
                state.sendVisca("B/W", "bw", if (state.bw) 1 else 0)
            }
        }
    }
}

@Composable
private fun ApplyRow(label: String, value: String, onApply: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text(label, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
        Text(value, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Button(
            onClick = onApply,
            modifier = Modifier
                .defaultMinSize(minWidth = 64.dp, minHeight = 48.dp)
                .semantics { contentDescription = "Apply $label" }
        ) { Text("Apply") }
    }
}
