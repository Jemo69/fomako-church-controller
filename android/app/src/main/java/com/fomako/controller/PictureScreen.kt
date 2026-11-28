package com.fomako.controller

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt

private fun adjustColor(r: Float, g: Float, b: Float, bright: Int, contrast: Int, sat: Int): Color {
    val br = bright / 100f * 0.35f
    val co = 1f + contrast / 100f * 0.8f
    val sa = (1f + sat / 100f * 0.9f).coerceIn(0f, 2.5f)
    val rr = ((r - 0.5f) * co + 0.5f + br).coerceIn(0f, 1f)
    val gg = ((g - 0.5f) * co + 0.5f + br).coerceIn(0f, 1f)
    val bb = ((b - 0.5f) * co + 0.5f + br).coerceIn(0f, 1f)
    val l = 0.299f * rr + 0.587f * gg + 0.114f * bb
    return Color(
        red = (l + (rr - l) * sa).coerceIn(0f, 1f),
        green = (l + (gg - l) * sa).coerceIn(0f, 1f),
        blue = (l + (bb - l) * sa).coerceIn(0f, 1f),
        alpha = 1f
    )
}

@Composable
fun PictureScreen(state: ControllerState) {
    Text(
        text = "Picture Edit — preview, then Apply to Camera",
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.SemiBold,
    )
    Text(
        text = "Tune the look like a photo editor. Apply sends Brightness/Contrast/Saturation/Sharpness (VISCA A1/A2/49/42).",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )

    LivePreviewCard(state)

    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            SectionHeading("Adjust")
            PictureSlider("Brightness", state.picBright) { state.picBright = it }
            PictureSlider("Contrast", state.picContrast) { state.picContrast = it }
            PictureSlider("Saturation", state.picSat) { state.picSat = it }
            PictureSlider("Sharpness", state.picSharp) { state.picSharp = it }

            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(
                    onClick = { state.applyPictureEdit() },
                    modifier = Modifier.weight(1f).defaultMinSize(minHeight = 48.dp)
                ) { Text("Apply to Camera") }
                OutlinedButton(
                    onClick = {
                        state.picBright = 0
                        state.picContrast = 0
                        state.picSat = 0
                        state.picSharp = 0
                        state.setStatusMsg("Picture sliders reset (camera unchanged)")
                    },
                    modifier = Modifier.defaultMinSize(minHeight = 48.dp)
                ) { Text("Reset") }
            }
        }
    }

    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            SectionHeading("Live simulated preview")
            val cols = listOf(
                "R" to Triple(0.85f, 0.15f, 0.15f),
                "G" to Triple(0.15f, 0.70f, 0.20f),
                "B" to Triple(0.15f, 0.35f, 0.90f),
                "Y" to Triple(0.90f, 0.80f, 0.15f),
                "C" to Triple(0.15f, 0.80f, 0.85f),
                "M" to Triple(0.75f, 0.20f, 0.70f),
            )
            val sharpBorder = when {
                state.picSharp > 40 -> 3.dp
                state.picSharp > 0 -> 2.dp
                else -> 0.dp
            }
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                for ((name, rgb) in cols) {
                    val adj = adjustColor(rgb.first, rgb.second, rgb.third, state.picBright, state.picContrast, state.picSat)
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .height(78.dp)
                            .clip(RoundedCornerShape(8.dp))
                            .background(adj)
                            .then(
                                if (sharpBorder > 0.dp) {
                                    Modifier.border(sharpBorder, MaterialTheme.colorScheme.onSurface, RoundedCornerShape(8.dp))
                                } else Modifier
                            ),
                        contentAlignment = Alignment.Center
                    ) {
                        Text(name, color = Color.White, fontWeight = FontWeight.Bold)
                    }
                }
            }
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                for (i in 0..7) {
                    val g = i / 7f
                    val adj = adjustColor(g, g, g, state.picBright, state.picContrast, 0)
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .height(32.dp)
                            .clip(RoundedCornerShape(4.dp))
                            .background(adj)
                    )
                }
            }
            val values = pictureEditValues(state.picBright, state.picContrast, state.picSat, state.picSharp)
            Text(
                text = "Will send: Bright ${values.brightness}  Contrast ${values.contrast}  Sat ${values.saturation}  Sharp ${values.sharpness}",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontFamily = FontFamily.Monospace,
            )
        }
    }
}

@Composable
private fun PictureSlider(label: String, value: Int, onChange: (Int) -> Unit) {
    Column(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Text(label, style = MaterialTheme.typography.bodyMedium)
            Text(
                text = "%+d".format(value),
                style = MaterialTheme.typography.labelLarge,
                fontFamily = FontFamily.Monospace
            )
        }
        Slider(
            value = value.toFloat(),
            onValueChange = { onChange((it / 5f).roundToInt() * 5) },
            valueRange = -100f..100f,
            modifier = Modifier.semantics {
                contentDescription = label
                stateDescription = "%+d".format(value)
            }
        )
    }
}
