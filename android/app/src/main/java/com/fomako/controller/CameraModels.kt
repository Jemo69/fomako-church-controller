package com.fomako.controller

import android.content.Context
import android.net.wifi.WifiManager
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder

enum class NavTab(val title: String) {
    Control("Control"),
    Image("Image"),
    Picture("Picture"),
    Cameras("Cameras"),
    Help("Help"),
}

enum class ProtoMode(val jsonName: String, val label: String, val defaultPort: Int) {
    SonyUdp("SonyUdp", "Sony VISCA UDP :52381", 52381),
    Tcp("Tcp", "IP-VISCA TCP :5678", 5678);

    companion object {
        fun fromJson(s: String): ProtoMode =
            if (s == "Tcp") Tcp else SonyUdp
    }
}

data class CameraConfig(
    val name: String,
    val ip: String,
    val protocol: ProtoMode = ProtoMode.SonyUdp,
    val address: Int = 1,
    val username: String = "admin",
    val password: String = "admin",
)

data class ScanHit(
    val ip: String,
    val detail: String,
    val tcp5678: Boolean,
    val udp52381: Boolean,
    val onvif: Boolean,
)

internal data class PictureEditValues(val brightness: Int, val contrast: Int, val saturation: Int, val sharpness: Int)

internal fun pictureEditValues(bright: Int, contrast: Int, sat: Int, sharp: Int) = PictureEditValues(
    brightness = (7.0f + bright * 0.07f).let { kotlin.math.round(it).toInt().coerceIn(0, 14) },
    contrast = (7.0f + contrast * 0.07f).let { kotlin.math.round(it).toInt().coerceIn(0, 14) },
    saturation = (4.0f + sat * 0.10f).let { kotlin.math.round(it).toInt().coerceIn(0, 14) },
    sharpness = (8.0f + sharp * 0.07f).let { kotlin.math.round(it).toInt().coerceIn(0, 15) },
)

private data class ViscaCommand(
    val camera: CameraConfig,
    val what: String,
    val payload: ByteArray,
    val relay: String?,
)

class ControllerState(private val context: Context) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val configFile = File(context.filesDir, "fomako-cameras.json")
    private val viscaQueue = Channel<ViscaCommand>(Channel.UNLIMITED)

    val cameras = mutableStateListOf<CameraConfig>()
    var selected by mutableIntStateOf(0)
    var tab by mutableStateOf(NavTab.Control)
    var dark by mutableStateOf(true)

    var panSpeed by mutableIntStateOf(14)
    var tiltSpeed by mutableIntStateOf(12)
    var zoomSpeed by mutableIntStateOf(4)
    var presetSel by mutableIntStateOf(1)

    var status by mutableStateOf("Ready. Pick a camera, then drive it.")
    val log = mutableStateListOf<String>()

    var previewOn by mutableStateOf(true)
    var previewStatus by mutableStateOf("Preview starting…")

    // Image settings (§5.2)
    var aeMode by mutableIntStateOf(0) // 0 auto, 1 manual, 2 shutter, 3 iris, 4 bright
    var shutter by mutableIntStateOf(10)
    var iris by mutableIntStateOf(8)
    var gainLimit by mutableIntStateOf(8)
    var brightLvl by mutableIntStateOf(8)
    var backlight by mutableStateOf(false)
    var wdr by mutableIntStateOf(0)
    var nr2d by mutableIntStateOf(3)
    var nr3d by mutableIntStateOf(5)
    var flicker by mutableIntStateOf(0)
    var gamma by mutableIntStateOf(2)
    var lowlight by mutableStateOf(false)
    var aperture by mutableIntStateOf(8)
    var brightness by mutableIntStateOf(7)
    var contrast by mutableIntStateOf(7)
    var saturation by mutableIntStateOf(4)
    var wbMode by mutableIntStateOf(0)
    var rGain by mutableIntStateOf(32)
    var bGain by mutableIntStateOf(32)
    var flip by mutableIntStateOf(0)
    var bw by mutableStateOf(false)

    // Picture Edit (-100..+100)
    var picBright by mutableIntStateOf(0)
    var picContrast by mutableIntStateOf(0)
    var picSat by mutableIntStateOf(0)
    var picSharp by mutableIntStateOf(0)

    // LAN discovery + optional relay
    var scanPrefix by mutableStateOf("")
    var scanning by mutableStateOf(false)
    var scanMessage by mutableStateOf("Not scanned yet. Tip: FoMaKo default lives on 192.168.5.x.")
    val scanResults = mutableStateListOf<ScanHit>()

    var useRelay by mutableStateOf(false)
    var relayUrl by mutableStateOf("")

    init {
        loadCameras()
        scanPrefix = try {
            RustBridge.nativeDetectPrefix()
        } catch (_: Throwable) {
            "192.168.5"
        }
        pushLog("FoMaKo Android controller ready (VISCA 52381 / 5678)")
        scope.launch(Dispatchers.IO) {
            for (command in viscaQueue) {
                val result = executeVisca(command)
                withContext(Dispatchers.Main) {
                    result.fold(
                        onSuccess = { msg ->
                            setStatusMsg("${command.camera.name} ${command.what} OK: $msg")
                        },
                        onFailure = { error ->
                            setStatusMsg("${command.camera.name} ${command.what} FAILED: ${error.message ?: "send failed"}")
                        },
                    )
                }
            }
        }
    }

    fun current(): CameraConfig {
        if (cameras.isEmpty()) return CameraConfig("Pulpit", "192.168.5.163")
        return cameras[selected.coerceIn(0, cameras.lastIndex)]
    }

    fun pushLog(msg: String) {
        log.add(msg)
        if (log.size > 40) {
            log.removeRange(0, log.size - 40)
        }
    }

    fun setStatusMsg(msg: String) {
        status = msg
        pushLog(msg)
    }

    fun aeLabel(): String = when (aeMode) {
        1 -> "Manual"
        2 -> "Shutter Pri"
        3 -> "Iris Pri"
        4 -> "Bright"
        else -> "Full Auto"
    }

    fun aeByte(): Int = when (aeMode) {
        1 -> 0x03
        2 -> 0x0A
        3 -> 0x0B
        4 -> 0x0D
        else -> 0x00
    }

    fun wbLabel(): String = when (wbMode) {
        0 -> "Auto"
        1 -> "3000K"
        2 -> "4000K"
        3 -> "OnePush"
        4 -> "OnePushTrig"
        5 -> "Manual"
        6 -> "6500K"
        else -> "?"
    }

    fun saveCameras() {
        try {
            val arr = JSONArray()
            for (c in cameras) {
                arr.put(
                    JSONObject().apply {
                        put("name", c.name)
                        put("ip", c.ip)
                        put("protocol", c.protocol.jsonName)
                        put("address", c.address)
                        put("username", c.username)
                        put("password", c.password)
                    }
                )
            }
            configFile.writeText(arr.toString(2))
        } catch (_: Throwable) {
        }
    }

    private fun loadCameras() {
        cameras.clear()
        if (configFile.isFile) {
            try {
                val arr = JSONArray(configFile.readText())
                for (i in 0 until arr.length()) {
                    val o = arr.getJSONObject(i)
                    cameras.add(
                        CameraConfig(
                            name = o.optString("name", "Camera ${i + 1}"),
                            ip = o.optString("ip", "192.168.5.163"),
                            protocol = ProtoMode.fromJson(o.optString("protocol", "SonyUdp")),
                            address = o.optInt("address", 1).coerceIn(1, 7),
                            username = o.optString("username", "admin"),
                            password = o.optString("password", "admin"),
                        )
                    )
                }
            } catch (_: Throwable) {
            }
        }
        if (cameras.isEmpty()) {
            cameras.add(CameraConfig("Pulpit", "192.168.5.163", ProtoMode.SonyUdp, 1, "admin", "admin"))
            cameras.add(CameraConfig("Wide / Choir", "192.168.5.164", ProtoMode.SonyUdp, 2, "admin", "admin"))
        }
    }

    fun sendVisca(what: String, op: String, a: Int = 0, b: Int = 0, c: Int = 0) {
        val cam = current()
        val pkt = try {
            RustBridge.nativeBuildVisca(op, cam.address, a, b, c)
        } catch (error: Throwable) {
            setStatusMsg("${cam.name} $what FAILED: ${error.message ?: "VISCA builder unavailable"}")
            return
        }
        if (pkt.isEmpty()) {
            setStatusMsg("${cam.name} $what FAILED: unsupported VISCA command '$op'")
            return
        }
        val hex = pkt.joinToString(" ") { "%02X".format(it) }
        val relay = relayUrl.trim().trimEnd('/').takeIf { useRelay && it.isNotEmpty() }
        val via = relay?.let { " via relay" }.orEmpty()
        setStatusMsg("${cam.name} -> $what (${cam.ip}$via): $hex")
        val queued = viscaQueue.trySend(ViscaCommand(cam, what, pkt, relay))
        if (!queued.isSuccess) {
            setStatusMsg("${cam.name} $what FAILED: command queue unavailable")
        }
    }

    private fun executeVisca(command: ViscaCommand): Result<String> {
        val relay = command.relay
        if (relay != null) {
            val error = postRelayVisca(relay, command.camera, command.payload)
            return if (error == null) Result.success("sent via relay") else Result.failure(IllegalStateException(error))
        }
        return runCatching {
            val raw = RustBridge.nativeSendVisca(
                command.camera.ip,
                command.camera.protocol.defaultPort,
                command.camera.protocol == ProtoMode.Tcp,
                command.payload,
            )
            val reply = JSONObject(raw)
            if (!reply.optBoolean("ok", false)) {
                error(reply.optString("error", "send failed"))
            }
            reply.optString("msg", "sent")
        }
    }

    fun testCamera(index: Int) {
        val cam = cameras.getOrNull(index) ?: return
        val pkt = RustBridge.nativeBuildVisca("pt_drive", cam.address, 8, 8, 9)
        setStatusMsg("Testing ${cam.name} (${cam.ip})…")
        scope.launch(Dispatchers.IO) {
            val raw = RustBridge.nativeSendVisca(
                cam.ip,
                cam.protocol.defaultPort,
                cam.protocol == ProtoMode.Tcp,
                pkt
            )
            val reply = runCatching { JSONObject(raw) }.getOrNull()
            withContext(Dispatchers.Main) {
                if (reply != null && reply.optBoolean("ok", false)) {
                    setStatusMsg("Test ${cam.name} OK: ${reply.optString("msg")} [${reply.optString("hex")}]")
                } else {
                    val err = reply?.optString("error") ?: "unknown error"
                    setStatusMsg("Test ${cam.name} FAILED (${cam.ip}): $err")
                }
            }
        }
    }

    fun startScan() {
        if (scanning) return
        val raw = scanPrefix.trim().replace('/', ' ').split(Regex("\\s+")).firstOrNull().orEmpty()
        val parts = raw.split('.')
        val prefix = if (parts.size == 4 && parts[3] == "0") parts.take(3).joinToString(".") else raw
        val octets = prefix.split('.')
        if (octets.size != 3 || octets.any { it.toIntOrNull()?.let { n -> n in 0..255 } != true }) {
            setStatusMsg("Scan: enter subnet as 3 numbers, e.g. 192.168.1")
            return
        }
        scanning = true
        scanResults.clear()
        val relay = relayUrl.trim().trimEnd('/')
        val viaRelay = useRelay && relay.isNotEmpty()
        scanMessage = if (viaRelay) "Scanning $prefix.1-254 via relay…" else "Scanning $prefix.1-254 + ONVIF…"
        pushLog("Scan started on $prefix.0/24")

        scope.launch(Dispatchers.IO) {
            val hits: List<ScanHit> = if (viaRelay) {
                getRelayScan(relay, prefix)
            } else {
                val wifi = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
                val lock = wifi?.createMulticastLock("fomako_onvif")?.apply {
                    setReferenceCounted(false)
                    runCatching { acquire() }
                }
                try {
                    val json = RustBridge.nativeScanSubnet(prefix)
                    parseScanArray(json)
                } finally {
                    runCatching {
                        if (lock?.isHeld == true) lock.release()
                    }
                }
            }
            withContext(Dispatchers.Main) {
                scanning = false
                scanResults.clear()
                scanResults.addAll(hits)
                scanMessage = if (hits.isEmpty()) {
                    "No cameras found on $prefix.x — same WiFi as the camera? FoMaKo default lives on 192.168.5.x."
                } else {
                    "Found ${hits.size} camera${if (hits.size == 1) "" else "s"} on $prefix.x — tap Add."
                }
                pushLog("Scan finished: ${hits.size} found")
            }
        }
    }

    fun applyPictureEdit() {
        val values = pictureEditValues(picBright, picContrast, picSat, picSharp)
        brightness = values.brightness
        contrast = values.contrast
        saturation = values.saturation
        aperture = values.sharpness
        sendVisca("Picture Brightness ${values.brightness}", "brightness", values.brightness)
        sendVisca("Picture Contrast ${values.contrast}", "contrast", values.contrast)
        sendVisca("Picture Saturation ${values.saturation}", "saturation", values.saturation)
        sendVisca("Picture Sharpness ${values.sharpness}", "aperture", values.sharpness)
    }

    private fun parseScanArray(json: String): List<ScanHit> {
        val out = mutableListOf<ScanHit>()
        runCatching {
            val arr = JSONArray(json)
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                out.add(
                    ScanHit(
                        ip = o.getString("ip"),
                        detail = o.optString("detail", ""),
                        tcp5678 = o.optBoolean("tcp5678", false),
                        udp52381 = o.optBoolean("udp52381", false),
                        onvif = o.optBoolean("onvif", false),
                    )
                )
            }
        }
        return out
    }

    private fun postRelayVisca(relay: String, cam: CameraConfig, pkt: ByteArray): String? {
        return runCatching {
            val conn = (URL("$relay/visca").openConnection() as HttpURLConnection).apply {
                requestMethod = "POST"
                connectTimeout = 2000
                readTimeout = 3000
                doOutput = true
                setRequestProperty("Content-Type", "application/json")
            }
            val hex = pkt.joinToString("") { "%02x".format(it) }
            val body = JSONObject().apply {
                put("ip", cam.ip)
                put("port", cam.protocol.defaultPort)
                put("proto", if (cam.protocol == ProtoMode.Tcp) "tcp" else "sony_udp")
                put("payload", hex)
            }.toString()
            conn.outputStream.use { it.write(body.toByteArray()) }
            val code = conn.responseCode
            val text = (if (code in 200..299) conn.inputStream else conn.errorStream)
                ?.bufferedReader()?.readText().orEmpty()
            if (code !in 200..299) "relay HTTP $code: $text" else null
        }.getOrElse { "relay error: ${it.message}" }
    }

    private fun getRelayScan(relay: String, prefix: String): List<ScanHit> {
        return runCatching {
            val q = URLEncoder.encode(prefix, "UTF-8")
            val conn = (URL("$relay/api/scan?subnet=$q").openConnection() as HttpURLConnection).apply {
                connectTimeout = 2000
                readTimeout = 30000
            }
            val text = conn.inputStream.bufferedReader().readText()
            val found = JSONObject(text).optJSONArray("found") ?: return emptyList()
            val list = mutableListOf<ScanHit>()
            for (i in 0 until found.length()) {
                val o = found.getJSONObject(i)
                val tcp = o.optBoolean("tcp5678", false)
                val udp = o.optBoolean("udp52381", false)
                val onvif = o.optBoolean("onvif", false)
                val hint = o.optString("http_hint", "")
                val bits = buildList {
                    if (tcp) add("VISCA :5678")
                    if (udp) add("Sony :52381")
                    if (onvif) add("ONVIF")
                    if (hint.isNotEmpty()) add("web \"$hint\"") else add("web :80")
                }
                list.add(ScanHit(o.getString("ip"), bits.joinToString(" · "), tcp, udp, onvif))
            }
            list
        }.getOrDefault(emptyList())
    }
}
