--- src/hooks/useCaptionSession.ts
+++ src/hooks/useCaptionSession.ts
@@ -10,13 +10,12 @@
     const [isInitializing, setIsInitializing] = useState(false);

     // Refs to hold instances across renders
-    // We instantiate the service lazily or once.
-    const serviceRef = useRef<TranscriptionService>(new TranscriptionService());
     const audioContextRef = useRef<AudioContext | null>(null);
     const streamRef = useRef<MediaStream | null>(null);
     const processorRef = useRef<AudioWorkletNode | null>(null);
     const sourceRef = useRef<MediaStreamAudioSourceNode | null>(null);

     // Native capture refs
     const usingNativeCaptureRef = useRef(false);
     const systemAudioUnlistenRef = useRef<UnlistenFn | null>(null);
@@ -32,24 +31,24 @@
     // Helper to update service config from AppConfig
-    const updateServiceConfig = useCallback(async (service: TranscriptionService, cfg: AppConfig) => {
-        service.setModelPath(cfg.offlineModelPath);
-        service.setLanguage(cfg.language);
-        service.setEnableITN(cfg.enableITN ?? false);
-        service.setPunctuationModelPath(cfg.punctuationModelPath || '');
-        service.setVadModelPath(cfg.vadModelPath || '');
-        service.setVadBufferSize(cfg.vadBufferSize || 5);
+    const updateServiceConfig = useCallback(async (cfg: AppConfig) => {
+        transcriptionService.setModelPath(cfg.offlineModelPath);
+        transcriptionService.setLanguage(cfg.language);
+        transcriptionService.setEnableITN(cfg.enableITN ?? false);
+        transcriptionService.setPunctuationModelPath(cfg.punctuationModelPath || '');
+        transcriptionService.setVadModelPath(cfg.vadModelPath || '');
+        transcriptionService.setVadBufferSize(cfg.vadBufferSize || 5);

         // ITN Setup
         const enabledITNModels = new Set(cfg.enabledITNModels || []);
         const itnRulesOrder = cfg.itnRulesOrder || ['itn-zh-number'];
         if (enabledITNModels.size > 0) {
             try {
                 const paths = await modelService.getEnabledITNModelPaths(enabledITNModels, itnRulesOrder);
-                service.setITNModelPaths(paths);
+                transcriptionService.setITNModelPaths(paths);
             } catch (e) {
                 console.warn('[CaptionSession] Failed to setup ITN paths:', e);
-                service.setITNModelPaths([]);
+                transcriptionService.setITNModelPaths([]);
             }
         } else {
-            service.setITNModelPaths([]);
+            transcriptionService.setITNModelPaths([]);
         }
     }, []); // Stable callback
