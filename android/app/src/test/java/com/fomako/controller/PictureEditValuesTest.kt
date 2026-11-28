package com.fomako.controller

import org.junit.Assert.assertEquals
import org.junit.Test

class PictureEditValuesTest {
    @Test
    fun neutralSaturationIs100Percent() {
        val values = pictureEditValues(0, 0, 0, 0)
        assertEquals(7, values.brightness)
        assertEquals(7, values.contrast)
        assertEquals(4, values.saturation)
        assertEquals(8, values.sharpness)
    }

    @Test
    fun valuesStayInsideCameraRanges() {
        val low = pictureEditValues(-100, -100, -100, -100)
        assertEquals(0, low.brightness)
        assertEquals(0, low.contrast)
        assertEquals(0, low.saturation)
        assertEquals(1, low.sharpness)

        val high = pictureEditValues(100, 100, 100, 100)
        assertEquals(14, high.brightness)
        assertEquals(14, high.contrast)
        assertEquals(14, high.saturation)
        assertEquals(15, high.sharpness)
    }

    @Test
    fun rtspCredentialsAreUrlEncoded() {
        val camera = CameraConfig(
            name = "Test",
            ip = "192.168.5.163",
            username = "ad@min",
            password = "p:a/s?s#",
        )
        assertEquals(
            "rtsp://ad%40min:p%3Aa%2Fs%3Fs%23@192.168.5.163:554/1",
            buildRtspUrl(camera),
        )
    }
}
