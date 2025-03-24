build:
	@wasm-pack build --target web
clean:
	@cargo clean
	@rm -rf pkg
test: build
	python3 -m http.server
