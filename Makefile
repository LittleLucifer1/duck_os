all:
	@rm -rf os/.cargo
	@cp -r os/cargo-submit os/.cargo
	@cd os && make all