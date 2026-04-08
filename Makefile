SHELL_NAME := $(notdir $(SHELL))

.PHONY: install uninstall

install:
	cargo install --path .
	work setup $(SHELL_NAME)

uninstall:
	cargo uninstall work
