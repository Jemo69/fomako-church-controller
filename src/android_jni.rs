//! Android JNI bridge exposing FoMaKo VISCA builders, socket transport, and
//! LAN camera discovery (`src/visca.rs`, `src/discover.rs`) to the Kotlin app.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jint};
use serde::Serialize;

use crate::discover::{self, ScanEvent, ScanHandle};
use crate::visca::{self, Protocol};

#[derive(Serialize)]
struct SendReply {
    ok: bool,
    msg: String,
    error: String,
    hex: String,
}

#[derive(Serialize)]
struct ScanItem {
    ip: String,
    detail: String,
    tcp5678: bool,
    udp52381: bool,
    onvif: bool,
}

fn build_visca_packet(op: &str, addr: u8, a: i32, b: i32, c: i32) -> Vec<u8> {
    visca::named_command(op, addr, a, b, c).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_fomako_controller_RustBridge_nativeBuildVisca<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    op: JString<'caller>,
    addr: jint,
    a: jint,
    b: jint,
    c: jint,
) -> JByteArray<'caller> {
    unowned_env
        .with_env(|env| -> Result<JByteArray<'caller>, jni::errors::Error> {
            let op_str = op.try_to_string(env)?;
            let pkt = build_visca_packet(&op_str, addr.clamp(1, 7) as u8, a, b, c);
            env.byte_array_from_slice(&pkt)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_fomako_controller_RustBridge_nativeSendVisca<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    ip: JString<'caller>,
    port: jint,
    is_tcp: jboolean,
    payload: JByteArray<'caller>,
) -> JString<'caller> {
    unowned_env
        .with_env(|env| -> Result<JString<'caller>, jni::errors::Error> {
            let ip_str = ip.try_to_string(env)?;
            let bytes = env.convert_byte_array(&payload)?;
            let proto = if is_tcp {
                Protocol::IpViscaTcp
            } else {
                Protocol::SonyUdp
            };
            let hex = visca::hex(&bytes);
            let reply = match visca::send_blocking(&ip_str, port.clamp(1, 65535) as u16, proto, &bytes) {
                Ok(msg) => SendReply {
                    ok: true,
                    msg,
                    error: String::new(),
                    hex,
                },
                Err(error) => SendReply {
                    ok: false,
                    msg: String::new(),
                    error,
                    hex,
                },
            };
            let json = serde_json::to_string(&reply).unwrap_or_else(|_| {
                "{\"ok\":false,\"msg\":\"\",\"error\":\"serialize failed\",\"hex\":\"\"}".into()
            });
            JString::from_str(env, json)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_fomako_controller_RustBridge_nativeDetectPrefix<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
) -> JString<'caller> {
    unowned_env
        .with_env(|env| -> Result<JString<'caller>, jni::errors::Error> {
            JString::from_str(env, discover::detect_prefix())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_fomako_controller_RustBridge_nativeScanSubnet<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    prefix: JString<'caller>,
) -> JString<'caller> {
    unowned_env
        .with_env(|env| -> Result<JString<'caller>, jni::errors::Error> {
            let prefix_str = prefix.try_to_string(env)?;
            let queue = Arc::new(Mutex::new(Vec::new()));
            let handle = Arc::new(ScanHandle::default());
            discover::scan_subnet(&prefix_str, queue.clone(), handle);

            let events = queue
                .lock()
                .map(|mut q| std::mem::take(&mut *q))
                .unwrap_or_default();
            let mut merged: Vec<ScanItem> = Vec::new();
            for ev in events {
                let ScanEvent::Found(f) = ev;
                if let Some(existing) = merged.iter_mut().find(|item| item.ip == f.ip) {
                    existing.tcp5678 |= f.tcp5678;
                    existing.udp52381 |= f.udp52381;
                    existing.onvif |= f.onvif;
                    if existing.detail.is_empty() || f.onvif {
                        existing.detail = f.detail();
                    }
                } else {
                    merged.push(ScanItem {
                        ip: f.ip.clone(),
                        detail: f.detail(),
                        tcp5678: f.tcp5678,
                        udp52381: f.udp52381,
                        onvif: f.onvif,
                    });
                }
            }
            merged.sort_by(|a, b| {
                let parse = |s: &str| -> [u8; 4] {
                    let mut p = s.split('.').filter_map(|x| x.parse::<u8>().ok());
                    [
                        p.next().unwrap_or(0),
                        p.next().unwrap_or(0),
                        p.next().unwrap_or(0),
                        p.next().unwrap_or(0),
                    ]
                };
                parse(&a.ip).cmp(&parse(&b.ip))
            });
            let _ = Ordering::SeqCst;
            let json = serde_json::to_string(&merged).unwrap_or_else(|_| "[]".into());
            JString::from_str(env, json)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
