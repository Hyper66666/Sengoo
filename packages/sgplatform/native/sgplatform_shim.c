#define SDL_MAIN_HANDLED
#if defined(SGPLATFORM_STUB)
#include <stdint.h>
#elif __has_include(<SDL.h>)
#include <SDL.h>
#include <stdint.h>
#elif __has_include(<SDL2/SDL.h>)
#include <SDL2/SDL.h>
#include <stdint.h>
#else
#error "SDL2 headers not found. Install SDL2 development headers or set SGPLATFORM_SKIP_GRAPHICS=1."
#include <stdint.h>
#endif

#if defined(_WIN32)
#define SGP_EXPORT __declspec(dllexport)
#else
#define SGP_EXPORT
#endif

#define SGP_STATUS_OK 0
#define SGP_STATUS_INVALID_ARGUMENT 2
#define SGP_STATUS_INVALID_HANDLE 3
#define SGP_STATUS_UNSUPPORTED 8
#define SGP_STATUS_IO 9
#define SGP_STATUS_PLATFORM 19

#define SGP_EVENT_NONE 0
#define SGP_EVENT_QUIT 1
#define SGP_EVENT_KEY_DOWN 2
#define SGP_EVENT_KEY_UP 3
#define SGP_EVENT_MOUSE_BUTTON_DOWN 4
#define SGP_EVENT_MOUSE_BUTTON_UP 5

#if defined(SGPLATFORM_STUB)

SGP_EXPORT int64_t sgplatform_init(void) {
    return (int64_t)SGP_STATUS_PLATFORM;
}

SGP_EXPORT int64_t sgplatform_quit(void) {
    return (int64_t)SGP_STATUS_OK;
}

SGP_EXPORT int64_t sgplatform_create_window(const char *title, int64_t width, int64_t height) {
    (void)title;
    (void)width;
    (void)height;
    return 0;
}

SGP_EXPORT int64_t sgplatform_destroy_window(int64_t window_handle) {
    (void)window_handle;
    return (int64_t)SGP_STATUS_INVALID_HANDLE;
}

SGP_EXPORT int64_t sgplatform_create_renderer(int64_t window_handle) {
    (void)window_handle;
    return 0;
}

SGP_EXPORT int64_t sgplatform_destroy_renderer(int64_t renderer_handle) {
    (void)renderer_handle;
    return (int64_t)SGP_STATUS_INVALID_HANDLE;
}

SGP_EXPORT int64_t sgplatform_poll_event(void) {
    return (int64_t)SGP_EVENT_NONE;
}

SGP_EXPORT int64_t sgplatform_last_event_key(void) {
    return 0;
}

SGP_EXPORT int64_t sgplatform_last_event_mouse_x(void) {
    return 0;
}

SGP_EXPORT int64_t sgplatform_last_event_mouse_y(void) {
    return 0;
}

SGP_EXPORT int64_t sgplatform_last_event_mouse_button(void) {
    return 0;
}

SGP_EXPORT int64_t sgplatform_ticks_ms(void) {
    return 0;
}

SGP_EXPORT int64_t sgplatform_delay_ms(int64_t ms) {
    return ms < 0 ? (int64_t)SGP_STATUS_INVALID_ARGUMENT : (int64_t)SGP_STATUS_OK;
}

SGP_EXPORT int64_t sgplatform_renderer_clear(int64_t renderer_handle, int64_t r, int64_t g, int64_t b, int64_t a) {
    (void)renderer_handle;
    (void)r;
    (void)g;
    (void)b;
    (void)a;
    return (int64_t)SGP_STATUS_INVALID_HANDLE;
}

SGP_EXPORT int64_t sgplatform_renderer_present(int64_t renderer_handle) {
    (void)renderer_handle;
    return (int64_t)SGP_STATUS_INVALID_HANDLE;
}

SGP_EXPORT int64_t sgplatform_renderer_draw_rect(
    int64_t renderer_handle,
    int64_t x,
    int64_t y,
    int64_t w,
    int64_t h,
    int64_t r,
    int64_t g,
    int64_t b,
    int64_t a) {
    (void)renderer_handle;
    (void)x;
    (void)y;
    (void)w;
    (void)h;
    (void)r;
    (void)g;
    (void)b;
    (void)a;
    return (int64_t)SGP_STATUS_INVALID_HANDLE;
}

SGP_EXPORT int64_t sgplatform_renderer_fill_rect(
    int64_t renderer_handle,
    int64_t x,
    int64_t y,
    int64_t w,
    int64_t h,
    int64_t r,
    int64_t g,
    int64_t b,
    int64_t a) {
    (void)renderer_handle;
    (void)x;
    (void)y;
    (void)w;
    (void)h;
    (void)r;
    (void)g;
    (void)b;
    (void)a;
    return (int64_t)SGP_STATUS_INVALID_HANDLE;
}

SGP_EXPORT int64_t sgplatform_renderer_draw_line(
    int64_t renderer_handle,
    int64_t x1,
    int64_t y1,
    int64_t x2,
    int64_t y2,
    int64_t r,
    int64_t g,
    int64_t b,
    int64_t a) {
    (void)renderer_handle;
    (void)x1;
    (void)y1;
    (void)x2;
    (void)y2;
    (void)r;
    (void)g;
    (void)b;
    (void)a;
    return (int64_t)SGP_STATUS_INVALID_HANDLE;
}

#else

static int g_initialized = 0;
static int64_t g_last_key = 0;
static int64_t g_last_mouse_x = 0;
static int64_t g_last_mouse_y = 0;
static int64_t g_last_mouse_button = 0;

static int64_t sgp_status_from_sdl(void) {
    return (int64_t)SGP_STATUS_PLATFORM;
}

static int64_t sgp_window_ptr(SDL_Window *window) {
    return (int64_t)(intptr_t)window;
}

static int64_t sgp_renderer_ptr(SDL_Renderer *renderer) {
    return (int64_t)(intptr_t)renderer;
}

static SDL_Window *sgp_window_from_handle(int64_t handle) {
    if (handle == 0) {
        return NULL;
    }
    return (SDL_Window *)(intptr_t)handle;
}

static SDL_Renderer *sgp_renderer_from_handle(int64_t handle) {
    if (handle == 0) {
        return NULL;
    }
    return (SDL_Renderer *)(intptr_t)handle;
}

static int64_t sgp_map_scancode(SDL_Scancode scancode) {
    return (int64_t)scancode;
}

static void sgp_store_mouse(const SDL_Event *event) {
    g_last_mouse_x = (int64_t)event->button.x;
    g_last_mouse_y = (int64_t)event->button.y;
    g_last_mouse_button = (int64_t)event->button.button;
}

SGP_EXPORT int64_t sgplatform_init(void) {
    if (g_initialized) {
        return (int64_t)SGP_STATUS_OK;
    }
    if (SDL_Init(SDL_INIT_VIDEO) != 0) {
        return sgp_status_from_sdl();
    }
    g_initialized = 1;
    g_last_key = 0;
    g_last_mouse_x = 0;
    g_last_mouse_y = 0;
    g_last_mouse_button = 0;
    return (int64_t)SGP_STATUS_OK;
}

SGP_EXPORT int64_t sgplatform_quit(void) {
    if (!g_initialized) {
        return (int64_t)SGP_STATUS_OK;
    }
    SDL_Quit();
    g_initialized = 0;
    return (int64_t)SGP_STATUS_OK;
}

SGP_EXPORT int64_t sgplatform_create_window(const char *title, int64_t width, int64_t height) {
    if (!g_initialized) {
        return 0;
    }
    if (title == NULL || width <= 0 || height <= 0) {
        return 0;
    }
    SDL_Window *window = SDL_CreateWindow(
        title,
        SDL_WINDOWPOS_CENTERED,
        SDL_WINDOWPOS_CENTERED,
        (int)width,
        (int)height,
        SDL_WINDOW_SHOWN);
    if (window == NULL) {
        return 0;
    }
    return sgp_window_ptr(window);
}

SGP_EXPORT int64_t sgplatform_destroy_window(int64_t window_handle) {
    SDL_Window *window = sgp_window_from_handle(window_handle);
    if (window == NULL) {
        return (int64_t)SGP_STATUS_INVALID_HANDLE;
    }
    SDL_DestroyWindow(window);
    return (int64_t)SGP_STATUS_OK;
}

SGP_EXPORT int64_t sgplatform_create_renderer(int64_t window_handle) {
    SDL_Window *window = sgp_window_from_handle(window_handle);
    if (window == NULL) {
        return 0;
    }
    SDL_Renderer *renderer = SDL_CreateRenderer(window, -1, SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC);
    if (renderer == NULL) {
        renderer = SDL_CreateRenderer(window, -1, SDL_RENDERER_SOFTWARE);
    }
    if (renderer == NULL) {
        return 0;
    }
    return sgp_renderer_ptr(renderer);
}

SGP_EXPORT int64_t sgplatform_destroy_renderer(int64_t renderer_handle) {
    SDL_Renderer *renderer = sgp_renderer_from_handle(renderer_handle);
    if (renderer == NULL) {
        return (int64_t)SGP_STATUS_INVALID_HANDLE;
    }
    SDL_DestroyRenderer(renderer);
    return (int64_t)SGP_STATUS_OK;
}

SGP_EXPORT int64_t sgplatform_poll_event(void) {
    SDL_Event event;
    g_last_key = 0;
    g_last_mouse_x = 0;
    g_last_mouse_y = 0;
    g_last_mouse_button = 0;

    while (SDL_PollEvent(&event)) {
        switch (event.type) {
        case SDL_QUIT:
            return (int64_t)SGP_EVENT_QUIT;
        case SDL_KEYDOWN:
            g_last_key = sgp_map_scancode(event.key.keysym.scancode);
            return (int64_t)SGP_EVENT_KEY_DOWN;
        case SDL_KEYUP:
            g_last_key = sgp_map_scancode(event.key.keysym.scancode);
            return (int64_t)SGP_EVENT_KEY_UP;
        case SDL_MOUSEBUTTONDOWN:
            sgp_store_mouse(&event);
            return (int64_t)SGP_EVENT_MOUSE_BUTTON_DOWN;
        case SDL_MOUSEBUTTONUP:
            sgp_store_mouse(&event);
            return (int64_t)SGP_EVENT_MOUSE_BUTTON_UP;
        default:
            break;
        }
    }
    return (int64_t)SGP_EVENT_NONE;
}

SGP_EXPORT int64_t sgplatform_last_event_key(void) {
    return g_last_key;
}

SGP_EXPORT int64_t sgplatform_last_event_mouse_x(void) {
    return g_last_mouse_x;
}

SGP_EXPORT int64_t sgplatform_last_event_mouse_y(void) {
    return g_last_mouse_y;
}

SGP_EXPORT int64_t sgplatform_last_event_mouse_button(void) {
    return g_last_mouse_button;
}

SGP_EXPORT int64_t sgplatform_ticks_ms(void) {
    return (int64_t)SDL_GetTicks();
}

SGP_EXPORT int64_t sgplatform_delay_ms(int64_t ms) {
    if (ms < 0) {
        return (int64_t)SGP_STATUS_INVALID_ARGUMENT;
    }
    SDL_Delay((Uint32)ms);
    return (int64_t)SGP_STATUS_OK;
}

static int64_t sgp_renderer_set_color(SDL_Renderer *renderer, int64_t r, int64_t g, int64_t b, int64_t a) {
    if (renderer == NULL) {
        return (int64_t)SGP_STATUS_INVALID_HANDLE;
    }
    if (r < 0 || r > 255 || g < 0 || g > 255 || b < 0 || b > 255 || a < 0 || a > 255) {
        return (int64_t)SGP_STATUS_INVALID_ARGUMENT;
    }
    if (SDL_SetRenderDrawColor(renderer, (Uint8)r, (Uint8)g, (Uint8)b, (Uint8)a) != 0) {
        return sgp_status_from_sdl();
    }
    return (int64_t)SGP_STATUS_OK;
}

SGP_EXPORT int64_t sgplatform_renderer_clear(int64_t renderer_handle, int64_t r, int64_t g, int64_t b, int64_t a) {
    SDL_Renderer *renderer = sgp_renderer_from_handle(renderer_handle);
    int64_t color_status = sgp_renderer_set_color(renderer, r, g, b, a);
    if (color_status != SGP_STATUS_OK) {
        return color_status;
    }
    if (SDL_RenderClear(renderer) != 0) {
        return sgp_status_from_sdl();
    }
    return (int64_t)SGP_STATUS_OK;
}

SGP_EXPORT int64_t sgplatform_renderer_present(int64_t renderer_handle) {
    SDL_Renderer *renderer = sgp_renderer_from_handle(renderer_handle);
    if (renderer == NULL) {
        return (int64_t)SGP_STATUS_INVALID_HANDLE;
    }
    SDL_RenderPresent(renderer);
    return (int64_t)SGP_STATUS_OK;
}

SGP_EXPORT int64_t sgplatform_renderer_draw_rect(
    int64_t renderer_handle,
    int64_t x,
    int64_t y,
    int64_t w,
    int64_t h,
    int64_t r,
    int64_t g,
    int64_t b,
    int64_t a) {
    SDL_Renderer *renderer = sgp_renderer_from_handle(renderer_handle);
    int64_t color_status = sgp_renderer_set_color(renderer, r, g, b, a);
    if (color_status != SGP_STATUS_OK) {
        return color_status;
    }
    SDL_Rect rect = {(int)x, (int)y, (int)w, (int)h};
    if (SDL_RenderDrawRect(renderer, &rect) != 0) {
        return sgp_status_from_sdl();
    }
    return (int64_t)SGP_STATUS_OK;
}

SGP_EXPORT int64_t sgplatform_renderer_fill_rect(
    int64_t renderer_handle,
    int64_t x,
    int64_t y,
    int64_t w,
    int64_t h,
    int64_t r,
    int64_t g,
    int64_t b,
    int64_t a) {
    SDL_Renderer *renderer = sgp_renderer_from_handle(renderer_handle);
    int64_t color_status = sgp_renderer_set_color(renderer, r, g, b, a);
    if (color_status != SGP_STATUS_OK) {
        return color_status;
    }
    SDL_Rect rect = {(int)x, (int)y, (int)w, (int)h};
    if (SDL_RenderFillRect(renderer, &rect) != 0) {
        return sgp_status_from_sdl();
    }
    return (int64_t)SGP_STATUS_OK;
}

SGP_EXPORT int64_t sgplatform_renderer_draw_line(
    int64_t renderer_handle,
    int64_t x1,
    int64_t y1,
    int64_t x2,
    int64_t y2,
    int64_t r,
    int64_t g,
    int64_t b,
    int64_t a) {
    SDL_Renderer *renderer = sgp_renderer_from_handle(renderer_handle);
    int64_t color_status = sgp_renderer_set_color(renderer, r, g, b, a);
    if (color_status != SGP_STATUS_OK) {
        return color_status;
    }
    if (SDL_RenderDrawLine(renderer, (int)x1, (int)y1, (int)x2, (int)y2) != 0) {
        return sgp_status_from_sdl();
    }
    return (int64_t)SGP_STATUS_OK;
}

#endif
