--- src/services/transcriptionService.ts
+++ src/services/transcriptionService.ts
@@ -53,8 +53,8 @@
     private vadBufferSize: number = 5;
     /** Whether to enable Inverse Text Normalization. */
     private enableITN: boolean = true;
-    /** Callback for new transcript segments. */
-    private onSegment: TranscriptionCallback | null = null;
-    /** Callback for error reporting. */
-    private onError: ErrorCallback | null = null;
+    /** Callbacks for new transcript segments. */
+    private onSegmentListeners: Set<TranscriptionCallback> = new Set();
+    /** Callbacks for error reporting. */
+    private onErrorListeners: Set<ErrorCallback> = new Set();
     /** Promise to track active starting to prevent race conditions. */
     private startingPromise: Promise<void> | null = null;
