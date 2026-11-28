package com.fomako.controller

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Button
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
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp

@Composable
fun ImageScreen(state: ControllerState) {
    val cam = state.current()
    Text(
        text = "Image settings — ${cam.name} @ ${cam.ip}",
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.SemiBold,
    )
    Text(
        text = "Pick a value with − / +, then tap Apply to send it to the camera. Tapping a mode chip sends it right away.",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )

    // Exposure
    SettingsCard("Exposure") {
        Text(
            text = "Mode: ${state.aeLabel()}",
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.SemiBold,
        )
        ModeChips(
            options = listOf("Auto" to 0, "Manual" to 1, "Shutter" to 2, "Iris" to 3, "Bright" to 4),
            selected = state.aeMode,
            onSelect = { code ->
                state.aeMode = code
                state.sendVisca("Exposure ${state.aeLabel()}", "ae_mode", state.aeByte())
            },
        )
        NumberRow(
            label = "Shutter speed",
            value = state.shutter,
            range = 0..20,
            hint = "Higher number = longer exposure",
            code = "VISCA 4A",
            onValue = { state.shutter = it },
            onApply = { state.sendVisca("Shutter", "shutter", state.shutter) },
        )
        NumberRow(
            label = "Iris",
            value = state.iris,
            range = 0..20,
            hint = "Higher number = more light in",
            code = "VISCA 4B",
            onValue = { state.iris = it },
            onApply = { state.sendVisca("Iris", "iris", state.iris) },
        )
        NumberRow(
            label = "Gain limit",
            value = state.gainLimit,
            range = 0..15,
            hint = "Caps how much the camera brightens a dark scene",
            code = "VISCA 2C",
            onValue = { state.gainLimit = it },
            onApply = { state.sendVisca("Gain Limit", "gain_limit", state.gainLimit) },
        )
        NumberRow(
            label = "Bright level",
            value = state.brightLvl,
            range = 0..23,
            hint = "Overall brightness in Bright exposure mode",
            code = "VISCA 4D",
            onValue = { state.brightLvl = it },
            onApply = { state.sendVisca("Bright Level", "bright", state.brightLvl) },
        )
    }

    // Backlight / WDR / NR
    SettingsCard("Backlight / WDR / noise reduction") {
        FlagRow(
            label = "Backlight compensation",
            value = if (state.backlight) "On" else "Off",
            action = if (state.backlight) "Turn off" else "Turn on",
        ) {
            state.backlight = !state.backlight
            state.sendVisca("Backlight", "backlight", if (state.backlight) 1 else 0)
        }
        NumberRow(
            label = "WDR (wide dynamic range)",
            value = state.wdr,
            range = 0..7,
            hint = "Brightens shadowed faces against a bright window",
            code = "VISCA 51",
            onValue = { state.wdr = it },
            onApply = { state.sendVisca("WDR", "wdr", state.wdr) },
        )
        NumberRow(
            label = "Noise reduction (2D)",
            value = state.nr2d,
            range = 0..7,
            hint = "Removes grain in flat areas",
            code = "VISCA 53",
            onValue = { state.nr2d = it },
            onApply = { state.sendVisca("NR 2D", "nr2d", state.nr2d) },
        )
        NumberRow(
            label = "Noise reduction (3D)",
            value = state.nr3d,
            range = 0..8,
            hint = "Removes grain in motion; high values smear movement",
            code = "VISCA 54",
            onValue = { state.nr3d = it },
            onApply = { state.sendVisca("NR 3D", "nr3d", state.nr3d) },
        )
    }

    // White balance / color
    SettingsCard("White balance / color") {
        Text(
            text = "Mode: ${state.wbLabel()}",
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = FontWeight.SemiBold,
        )
        ModeChips(
            options = listOf("Auto" to 0, "3000K" to 1, "4000K" to 2, "1-Push" to 3, "Manual" to 5, "6500K" to 6),
            selected = state.wbMode,
            onSelect = { code ->
                state.wbMode = code
                state.sendVisca("WB ${state.wbLabel()}", "wb_mode", code)
            },
        )
        NumberRow(
            label = "Red gain",
            value = state.rGain,
            range = 0..255,
            hint = "Fine-tunes colour in Manual white balance",
            code = "VISCA 43",
            onValue = { state.rGain = it },
            onApply = { state.sendVisca("R Gain", "rgain", state.rGain) },
        )
        NumberRow(
            label = "Blue gain",
            value = state.bGain,
            range = 0..255,
            hint = "Fine-tunes colour in Manual white balance",
            code = "VISCA 44",
            onValue = { state.bGain = it },
            onApply = { state.sendVisca("B Gain", "bgain", state.bGain) },
        )
    }

    // Picture & Misc
    SettingsCard("Picture & misc") {
        NumberRow(
            label = "Brightness",
            value = state.brightness,
            range = 0..14,
            hint = "Same as the Picture tab brightness slider",
            code = "VISCA A1",
            onValue = { state.brightness = it },
            onApply = { state.sendVisca("Brightness", "brightness", state.brightness) },
        )
        NumberRow(
            label = "Contrast",
            value = state.contrast,
            range = 0..14,
            hint = "Higher number = punchier picture",
            code = "VISCA A2",
            onValue = { state.contrast = it },
            onApply = { state.sendVisca("Contrast", "contrast", state.contrast) },
        )
        NumberRow(
            label = "Saturation",
            value = state.saturation,
            range = 0..14,
            hint = "0 = black and white, higher = more vivid colour",
            code = "VISCA 49",
            onValue = { state.saturation = it },
            onApply = { state.sendVisca("Saturation", "saturation", state.saturation) },
        )
        NumberRow(
            label = "Sharpness",
            value = state.aperture,
            range = 0..15,
            hint = "Detail enhancement; too high looks crunchy",
            code = "VISCA 42",
            onValue = { state.aperture = it },
            onApply = { state.sendVisca("Sharpness", "aperture", state.aperture) },
        )
        NumberRow(
            label = "Gamma",
            value = state.gamma,
            range = 0..4,
            hint = "Mid-tone contrast without clipping highlights",
            code = "VISCA 5B",
            onValue = { state.gamma = it },
            onApply = { state.sendVisca("Gamma", "gamma", state.gamma) },
        )
        FlagRow(
            label = "Flicker reduction",
            value = when (state.flicker) { 1 -> "50 Hz"; 2 -> "60 Hz"; else -> "Off" },
            action = when (state.flicker) { 1 -> "Set 60 Hz"; 2 -> "Turn off"; else -> "Set 50 Hz" },
        ) {
            state.flicker = (state.flicker + 1) % 3
            state.sendVisca("Flicker", "flicker", state.flicker)
        }
        FlagRow(
            label = "Low-light mode",
            value = if (state.lowlight) "On" else "Off",
            action = if (state.lowlight) "Turn off" else "Turn on",
        ) {
            state.lowlight = !state.lowlight
            state.sendVisca("Low-Light", "lowlight", if (state.lowlight) 1 else 0)
        }
        FlagRow(
            label = "Image flip",
            value = when (state.flip) { 1 -> "Horizontal"; 2 -> "Vertical"; 3 -> "Horizontal + vertical"; else -> "Off" },
            action = when (state.flip) { 1 -> "Next: vertical"; 2 -> "Next: both"; 3 -> "Turn off"; else -> "Next: horizontal" },
        ) {
            state.flip = (state.flip + 1) % 4
            state.sendVisca("Flip", "flip", state.flip)
        }
        FlagRow(
            label = "Picture mode",
            value = if (state.bw) "Black & white" else "Colour",
            action = if (state.bw) "Switch to colour" else "Switch to B&W",
        ) {
            state.bw = !state.bw
            state.sendVisca("B/W", "bw", if (state.bw) 1 else 0)
        }
    }
}

@Composable
private fun SettingsCard(title: String, content: @Composable () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            SectionHeading(title)
            content()
        }
    }
}

@Composable
private fun ModeChips(
    options: List<Pair<String, Int>>,
    selected: Int,
    onSelect: (Int) -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        for ((name, code) in options) {
            FilterChip(
                selected = selected == code,
                onClick = { onSelect(code) },
                label = { Text(name) }
            )
        }
    }
}

/**
 * One setting the user can dial and send: a readable name and explanation,
 * a plain number (no hex) between − and +, and the Apply button in the same
 * group so it is obvious which value it sends.
 */
@Composable
private fun NumberRow(
    label: String,
    value: Int,
    range: IntRange,
    hint: String,
    code: String,
    onValue: (Int) -> Unit,
    onApply: () -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        Text(label, style = MaterialTheme.typography.bodyLarge)
        Text(
            text = "$hint · range ${range.first}–${range.last} · $code",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            OutlinedButton(
                onClick = { onValue((value - 1).coerceAtLeast(range.first)) },
                enabled = value > range.first,
                modifier = Modifier
                    .defaultMinSize(minWidth = 56.dp, minHeight = 48.dp)
                    .semantics { contentDescription = "Decrease $label" },
            ) { Text("−") }
            Text(
                text = "$value",
                style = MaterialTheme.typography.titleMedium,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.SemiBold,
                textAlign = TextAlign.Center,
                modifier = Modifier.widthIn(min = 40.dp),
            )
            OutlinedButton(
                onClick = { onValue((value + 1).coerceAtMost(range.last)) },
                enabled = value < range.last,
                modifier = Modifier
                    .defaultMinSize(minWidth = 56.dp, minHeight = 48.dp)
                    .semantics { contentDescription = "Increase $label" },
            ) { Text("+") }
            Button(
                onClick = onApply,
                modifier = Modifier
                    .weight(1f)
                    .defaultMinSize(minWidth = 96.dp, minHeight = 48.dp)
                    .semantics { contentDescription = "Send $label to the camera" },
            ) { Text("Apply") }
        }
    }
}

/** On/off or cycling setting: shows the current state and names the next action. */
@Composable
private fun FlagRow(
    label: String,
    value: String,
    action: String,
    onToggle: () -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.bodyLarge)
            Text(
                text = "Now: $value",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Button(
            onClick = onToggle,
            modifier = Modifier
                .defaultMinSize(minHeight = 48.dp)
                .semantics { contentDescription = "$action: $label" },
        ) { Text(action) }
    }
}
