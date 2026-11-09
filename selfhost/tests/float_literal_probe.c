#define WIN32_LEAN_AND_MEAN
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fenv.h>

int main(int argc, char **argv) {
    if (argc != 2) return 64;
    FILE *input = NULL;
    if (fopen_s(&input, argv[1], "rb") || !input) return 65;
    if (fesetround(FE_TONEAREST)) return 66;
    char line[20000], clean[20000];
    while (fgets(line, sizeof(line), input)) {
        int bits = atoi(line);
        char *source = strchr(line, '\t');
        if (!source || (bits != 32 && bits != 64)) return 67;
        ++source;
        size_t used = 0;
        for (size_t i = 0; source[i] && source[i] != '\r' && source[i] != '\n'; ++i) {
            if (source[i] != '_') clean[used++] = source[i];
        }
        if (used >= 3 && (memcmp(clean + used - 3, "f32", 3) == 0 || memcmp(clean + used - 3, "f64", 3) == 0)) used -= 3;
        clean[used] = 0;
        char *end = NULL;
        if (bits == 64) {
            double value = strtod(clean, &end);
            int64_t pattern;
            memcpy(&pattern, &value, sizeof(pattern));
            if (end != clean + used) return 68;
            printf("ok %lld\n", (long long)pattern);
        } else {
            float value = strtof(clean, &end);
            uint32_t pattern;
            memcpy(&pattern, &value, sizeof(pattern));
            if (end != clean + used) return 69;
            printf("ok %llu\n", (unsigned long long)pattern);
        }
    }
    if (ferror(input)) return 70;
    fclose(input);
    return 0;
}
