--- src-tauri/src/sherpa.rs
+++ src-tauri/src/sherpa.rs
@@ -515,7 +515,9 @@
     let model_type = find_model_config(&model_path, enable_itn, &language)
         .ok_or_else(|| "Could not find valid model configuration".to_string())?;

-    let recognizer = Recognizer::new(model_type, num_threads, valid_itn)?;
+    let recognizer = tauri::async_runtime::spawn_blocking(move || {
+        Recognizer::new(model_type, num_threads, valid_itn)
+    }).await.map_err(|e| e.to_string())??;

     // Initialize Punctuation
     let mut punctuation = None;
@@ -530,6 +532,7 @@
         }
     }

     // Initialize VAD
     let mut vad = None;
     if let Some(v_path) = &vad_model {
