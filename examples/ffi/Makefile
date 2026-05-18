BUILD_DIR := ../build/ffi
CLANG ?= clang
EXEEXT ?=
RUNTIME_C := ../../tools/stdlib/runtime.c

.PHONY: all call-c c-calls-sengoo clean

all: call-c c-calls-sengoo

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

call-c: $(BUILD_DIR)
	cargo run -q -p sengoo-compiler --bin emit_ir -- sengoo_calls_c.sg $(BUILD_DIR)/sengoo_calls_c.ll
	$(CLANG) -Wno-override-module $(BUILD_DIR)/sengoo_calls_c.ll c_add.c $(RUNTIME_C) -o $(BUILD_DIR)/sengoo_calls_c$(EXEEXT)
	$(BUILD_DIR)/sengoo_calls_c$(EXEEXT)

c-calls-sengoo: $(BUILD_DIR)
	cargo run -q -p sengoo-compiler --bin emit_ir -- sengoo_exports.sg $(BUILD_DIR)/sengoo_exports.ll
	$(CLANG) -Wno-override-module c_calls_sengoo.c $(BUILD_DIR)/sengoo_exports.ll -o $(BUILD_DIR)/c_calls_sengoo$(EXEEXT)
	$(BUILD_DIR)/c_calls_sengoo$(EXEEXT)

clean:
	rm -rf $(BUILD_DIR)
