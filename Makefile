.PHONY: install build build-lambda build-sdk build-iac deploy synth diff destroy clean help

help:
	@echo "Untrusted Code Executor - Makefile Commands"
	@echo ""
	@echo "Setup:"
	@echo "  make install        Install all dependencies"
	@echo ""
	@echo "Build:"
	@echo "  make build          Build all packages (SDK, IAC, Lambda)"
	@echo "  make build-sdk      Build TypeScript SDK only"
	@echo "  make build-iac      Build CDK infrastructure only"
	@echo "  make build-lambda   Build Rust Lambda function only"
	@echo ""
	@echo "Deploy:"
	@echo "  make deploy         Deploy everything to AWS"
	@echo "  make synth          Synthesize CloudFormation template"
	@echo "  make diff           Show deployment differences"
	@echo "  make destroy        Destroy AWS resources"
	@echo ""
	@echo "Cleanup:"
	@echo "  make clean          Remove all build artifacts"

install:
	npm install

build:
	npm run build

build-sdk:
	npm run build:sdk

build-iac:
	npm run build:iac

build-lambda:
	npm run build:lambda

deploy:
	npm run deploy

synth:
	npm run synth

diff:
	npm run diff

destroy:
	npm run destroy

clean:
	npm run clean
