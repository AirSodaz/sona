#[cfg(target_os = "android")]
pub mod android_init {
    use jni::EnvUnowned;
    use jni::errors::LogErrorAndDefault;
    use jni::objects::{JClass, JObject};

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_sona_android_app_SonaApplication_initRustlsPlatformVerifier<
        'local,
    >(
        mut unowned_env: EnvUnowned<'local>,
        _class: JClass<'local>,
        context: JObject<'local>,
    ) -> bool {
        unowned_env
            .with_env(|env| -> jni::errors::Result<bool> {
                rustls_platform_verifier::android::init_with_env(env, context)?;
                log::info!("rustls-platform-verifier successfully initialized via JNI");
                Ok(true)
            })
            .resolve::<LogErrorAndDefault>()
    }
}
