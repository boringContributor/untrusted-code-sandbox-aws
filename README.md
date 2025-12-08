# Untrusted Code Executor

A secure sandbox for executing untrusted JavaScript code using AWS Lambda and Rust.

## Architecture

This project provides an idea for running arbitrary JavaScript code in a secure, isolated environment:

- **Runtime**: QuickJS embedded in a Rust Lambda function for fast, memory-safe execution
- **Sandbox**: Strict isolation with configurable memory limits, timeouts, and custom fetch implementation with domain allowlisting
- **SDK**: TypeScript client library for invoking the sandbox from any Node.js application
- **Infrastructure**: AWS CDK for automated deployment

## Repository Structure

```
packages/
├── lambda/     # Rust Lambda function with QuickJS runtime
├── sdk/        # TypeScript SDK for clients
└── iac/        # AWS CDK infrastructure code
```

## Quick Start

### Prerequisites

- Node.js 18+
- Rust and cargo-lambda
- AWS CLI configured with credentials

### Deploy

```bash
make install
make deploy
```

This will:
1. Install all dependencies
2. Build the Rust Lambda function
3. Synthesize the CDK stack
4. Deploy to your AWS account

### Use the SDK

```bash
npm install @untrusted-code/sdk
```

```typescript
import { runUntrustedCode } from '@untrusted-code/sdk';

const result = await runUntrustedCode({
  code: '2 + 2',
  functionName: 'your-deployed-function-name'
});

console.log(result.result); // 4
```

Or set the `SANDBOX_FUNCTION_NAME` environment variable:

```bash
export SANDBOX_FUNCTION_NAME=your-deployed-function-name
```

```typescript
const result = await runUntrustedCode({
  code: '2 + 2'
});
```

## Features

- **Memory Isolation**: Configurable memory limits (default 10MB, max 50MB)
- **Timeout Protection**: Configurable execution timeout (default 5s, max 25s)
- **Network Control**: Custom fetch() implementation with domain allowlisting for secure HTTP requests
- **Console Capture**: All console output captured and returned
- **Error Handling**: Comprehensive error reporting

## Security

The sandbox enforces strict security boundaries:

- No access to Node.js built-ins (require, import, process, fs, etc.)
- No access to Lambda environment or credentials
- Custom fetch() implementation with domain allowlisting (not native browser fetch)
- Network access disabled by default
- Private IP ranges blocked when network enabled
- Memory and CPU limits enforced by QuickJS and Lambda

## Development

```bash
# Show all available commands
make help

# Install dependencies
make install

# Build all packages
make build

# Build individual packages
make build-sdk       # Build SDK only
make build-iac       # Build infrastructure only
make build-lambda    # Build Lambda only

# Run tests
npm test

# Deploy to AWS
make deploy

# Show deployment diff
make diff

# Synthesize CloudFormation template
make synth

# Destroy AWS resources
make destroy

# Clean build artifacts
make clean
```

## License

MIT
