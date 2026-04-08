.PHONY: install uninstall

install:
	cargo install --path .
	work setup $$(basename $$SHELL)

uninstall:
	cargo uninstall work
