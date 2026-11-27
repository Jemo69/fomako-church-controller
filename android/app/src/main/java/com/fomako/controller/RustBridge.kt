package com.fomako.controller

object RustBridge {
    init {
        System.loadLibrary("fomako_church_controller")
    }

    external fun nativeBuildVisca(op: String, addr: Int, a: Int, b: Int, c: Int): ByteArray
    external fun nativeSendVisca(ip: String, port: Int, isTcp: Boolean, payload: ByteArray): String
    external fun nativeDetectPrefix(): String
    external fun nativeScanSubnet(prefix: String): String
}
