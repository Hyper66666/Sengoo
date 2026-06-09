// Global display/input state for the sggame phase-1 API.
#include <stdint.h>

static int64_t g_platform_alive = 0;
static int64_t g_window_handle = 0;
static int64_t g_window_closed = 1;
static int64_t g_renderer_handle = 0;
static int64_t g_renderer_destroyed = 1;
static int64_t g_surface_w = 0;
static int64_t g_surface_h = 0;
static int64_t g_keys_down[256];

int64_t sggame_state_platform_alive_get(void) {
    return g_platform_alive;
}

void sggame_state_platform_alive_set(int64_t value) {
    g_platform_alive = value;
}

int64_t sggame_state_window_handle_get(void) {
    return g_window_handle;
}

void sggame_state_window_handle_set(int64_t value) {
    g_window_handle = value;
}

int64_t sggame_state_window_closed_get(void) {
    return g_window_closed;
}

void sggame_state_window_closed_set(int64_t value) {
    g_window_closed = value;
}

int64_t sggame_state_renderer_handle_get(void) {
    return g_renderer_handle;
}

void sggame_state_renderer_handle_set(int64_t value) {
    g_renderer_handle = value;
}

int64_t sggame_state_renderer_destroyed_get(void) {
    return g_renderer_destroyed;
}

void sggame_state_renderer_destroyed_set(int64_t value) {
    g_renderer_destroyed = value;
}

int64_t sggame_state_surface_w_get(void) {
    return g_surface_w;
}

void sggame_state_surface_w_set(int64_t value) {
    g_surface_w = value;
}

int64_t sggame_state_surface_h_get(void) {
    return g_surface_h;
}

void sggame_state_surface_h_set(int64_t value) {
    g_surface_h = value;
}

int64_t sggame_state_key_down_get(int64_t key) {
    if (key < 0 || key >= 256) {
        return 0;
    }
    return g_keys_down[(int)key];
}

void sggame_state_key_down_set(int64_t key, int64_t down) {
    if (key < 0 || key >= 256) {
        return;
    }
    g_keys_down[(int)key] = down != 0 ? 1 : 0;
}

void sggame_state_keys_clear(void) {
    for (int i = 0; i < 256; ++i) {
        g_keys_down[i] = 0;
    }
}

void sggame_state_reset(void) {
    g_platform_alive = 0;
    g_window_handle = 0;
    g_window_closed = 1;
    g_renderer_handle = 0;
    g_renderer_destroyed = 1;
    g_surface_w = 0;
    g_surface_h = 0;
    for (int i = 0; i < 256; ++i) {
        g_keys_down[i] = 0;
    }
}
