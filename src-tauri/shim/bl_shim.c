/* BibleLive whisper.cpp shim — a tiny, stable C ABI over the parts of the
   whisper.cpp API we need. Compiled by the cc crate and linked against the
   static whisper/ggml libs produced by CMake (see build.rs). */

#include "whisper.h"

void *bl_init(const char *model_path) {
    struct whisper_context_params cparams = whisper_context_default_params();
    return (void *)whisper_init_from_file_with_params(model_path, cparams);
}

int bl_run(void *ctx, const float *samples, int n_samples, int n_threads) {
    struct whisper_full_params p =
        whisper_full_default_params(WHISPER_SAMPLING_GREEDY);
    p.n_threads = n_threads > 0 ? n_threads : 4;
    p.translate = false;
    p.no_context = true;
    p.no_timestamps = true;
    p.single_segment = false;
    p.print_special = false;
    p.print_progress = false;
    p.print_realtime = false;
    p.print_timestamps = false;
    p.suppress_blank = true;
    p.language = "en";
    if (whisper_full((struct whisper_context *)ctx, p, samples, n_samples) != 0) {
        return -1;
    }
    return whisper_full_n_segments((struct whisper_context *)ctx);
}

const char *bl_segment(void *ctx, int i) {
    return whisper_full_get_segment_text((struct whisper_context *)ctx, i);
}

void bl_free(void *ctx) { whisper_free((struct whisper_context *)ctx); }

/* ---- Crash diagnostics ------------------------------------------------
   Native crashes (access violations inside whisper/cpal/audio drivers) kill
   the process without Rust ever seeing a panic. Install an unhandled-
   exception filter that appends the fault code to %APPDATA%\BibleLive\crash.log. */

#include <windows.h>
#include <stdio.h>

static LONG WINAPI bl_crash_filter(EXCEPTION_POINTERS *ep) {
    const char *appdata = getenv("APPDATA");
    if (appdata) {
        char path[MAX_PATH];
        _snprintf(path, sizeof path, "%s/BibleLive/crash.log", appdata);
        FILE *f = fopen(path, "a");
        if (f) {
            fprintf(f, "native crash: code 0x%08lX at address %p\n",
                    (unsigned long)ep->ExceptionRecord->ExceptionCode,
                    (void *)ep->ExceptionRecord->ExceptionAddress);
            fclose(f);
        }
    }
    return EXCEPTION_EXECUTE_HANDLER;
}

void bl_install_crash_handler(void) {
    SetUnhandledExceptionFilter(bl_crash_filter);
}
