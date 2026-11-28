package com.fomako.controller

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

@Composable
fun HelpScreen() {
    Text(
        text = "Help — church setup in 5 minutes",
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.SemiBold,
    )
    HelpCard(
        "0. Direct Android control",
        "Join the same church WiFi as your FoMaKo cameras. This Android app sends Sony VISCA UDP (:52381) and IP-VISCA TCP (:5678) directly from your phone — no PC required."
    )
    HelpCard(
        "1. Live preview",
        "Control and Picture Edit fetch JPEG previews directly from http://camera/action_snapshot. If the camera asks for a login, set User & Password on the Cameras tab (FoMaKo default: admin / admin)."
    )
    HelpCard(
        "2. Lock the camera IP",
        "On the camera OSD (MENU on IR remote): Network -> turn DHCP OFF -> confirm. Otherwise the IP changes after a power cycle. Restore login with * # Manual on the remote."
    )
    HelpCard(
        "3. Control protocol",
        "Sony VISCA-over-IP UDP :52381 (recommended) or IP-VISCA TCP :5678. ONVIF discovery uses UDP multicast 239.255.255.250:3702."
    )
    HelpCard(
        "4. Sunday workflow",
        "Frame pulpit -> Preset No. 1 -> Set. Choir -> 2 -> Set. Wide -> 3 -> Set. During service tap Recall 0-9. Keep Pan 10-16 / Tilt 10-14 for smooth moves."
    )
    HelpCard(
        "5. Picture",
        "If dark/washed: Image -> Exposure Full Auto, then tune Picture Edit sliders -> Apply to Camera. Backlight ON for backlit pulpit; WDR 1-3 for bright windows."
    )
}

@Composable
private fun HelpCard(title: String, body: String) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            Text(title, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
            Text(body, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}
