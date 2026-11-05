//! Experimental reusable mobile host. App state, views and callbacks stay Rust.
//!
//! The safe application/event layer is platform-neutral. The iOS and experimental Android adapters supply native surfaces,
//! lifecycle and single-line editors. Neither is a production mobile SDK. This crate is separate so the main toolkit retains forbid(unsafe_code)
//! and unchanged default/desktop dependencies.
#![deny(unsafe_op_in_unsafe_fn)]
mod application;
mod protocol;
pub use application::{Application, HostContext};
pub use protocol::{PlatformEvent, HOST_ABI_VERSION};
pub use volt_ui_kit as ui_kit;

#[cfg(target_os = "ios")]
pub mod ios;

/// Export one Rust application factory to Volt's reusable iOS host.
///
/// The factory takes [`HostContext`] and returns `Result<Application<State>, String>`.
/// Use exactly once in a static-library app crate. The macro supplies the ABI
/// registration symbol; developers write no raw-pointer or UIKit code.
#[macro_export]
macro_rules! export_ios_app {
    ($factory:path) => {
        #[cfg(target_os = "ios")]
        #[no_mangle]
        pub extern "C" fn volt_app_register() -> bool {
            $crate::ios::register($factory)
        }
    };
}

#[cfg(target_os = "android")]
pub mod android;

/// Export the Android JNI entry points once from a `cdylib` application.
/// App screens, state and callbacks remain ordinary Rust.
#[macro_export]
macro_rules! export_android_app {
    ($factory:path) => {
        #[cfg(target_os = "android")]
        mod volt_android_exports {
            use super::*;
            use $crate::android::{
                self,
                jni::{
                    objects::{JByteArray, JClass, JObject, JString},
                    sys::{jint, jlong, jstring},
                    JNIEnv,
                },
            };
            #[no_mangle]
            pub extern "system" fn Java_dev_volt_host_VoltActivity_nativeVersion(
                _: JNIEnv<'_>,
                _: JClass<'_>,
            ) -> jint {
                $crate::HOST_ABI_VERSION as jint
            }
            #[no_mangle]
            pub extern "system" fn Java_dev_volt_host_VoltActivity_nativeCreate(
                mut env: JNIEnv<'_>,
                _: JClass<'_>,
                font: JByteArray<'_>,
            ) -> jlong {
                android::create(&mut env, font, $factory)
            }
            #[no_mangle]
            pub extern "system" fn Java_dev_volt_host_VoltActivity_nativeAttach(
                mut env: JNIEnv<'_>,
                _: JClass<'_>,
                id: jlong,
                surface: JObject<'_>,
            ) -> jstring {
                android::attach(&mut env, id, surface)
            }
            #[no_mangle]
            pub extern "system" fn Java_dev_volt_host_VoltActivity_nativeDetach(
                mut env: JNIEnv<'_>,
                _: JClass<'_>,
                id: jlong,
            ) {
                android::detach(&mut env, id);
            }
            #[no_mangle]
            pub extern "system" fn Java_dev_volt_host_VoltActivity_nativeEvent(
                mut env: JNIEnv<'_>,
                _: JClass<'_>,
                id: jlong,
                event: JString<'_>,
            ) -> jstring {
                android::event(&mut env, id, event)
            }
            #[no_mangle]
            pub extern "system" fn Java_dev_volt_host_VoltActivity_nativeDestroy(
                mut env: JNIEnv<'_>,
                _: JClass<'_>,
                id: jlong,
            ) {
                android::destroy(&mut env, id);
            }
            #[no_mangle]
            pub extern "system" fn Java_dev_volt_host_VoltActivity_nativeError(
                env: JNIEnv<'_>,
                _: JClass<'_>,
            ) -> jstring {
                android::last_error(env)
            }
        }
    };
}
