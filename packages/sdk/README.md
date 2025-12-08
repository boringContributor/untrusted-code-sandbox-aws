# @untrusted-code/sdk

TypeScript SDK for executing untrusted JavaScript code in AWS Lambda sandbox.

## Installation

```bash
npm install @untrusted-code/sdk
```

## Development

```bash
# Install dependencies
npm install

# Build the SDK
npm run build

# Run tests
npm test

# Run tests in watch mode
npm run test:watch
```

## Usage

### Basic Usage

```typescript
import { runUntrustedCode } from '@untrusted-code/sdk';

// Uses SANDBOX_FUNCTION_NAME env var
const result = await runUntrustedCode({
  code: '2 + 2'
});

console.log(result.result); // 4
console.log(result.success); // true
console.log(result.executionTimeMs); // e.g., 15
```

### With Explicit Function Name

```typescript
import { runUntrustedCode } from '@untrusted-code/sdk';

const result = await runUntrustedCode({
  code: 'console.log("Hello"); return 42;',
  functionName: 'your-lambda-function-name'
});

console.log(result.result); // 42
console.log(result.consoleOutput); // ['[log] Hello']
```

### With Timeout and Memory Limits

```typescript
const result = await runUntrustedCode({
  code: 'while(true) {}',
  timeoutMs: 1000,
  memoryLimitBytes: 10 * 1024 * 1024 // 10MB
});

console.log(result.success); // false
console.log(result.error); // 'Execution timeout exceeded'
```

### Error Handling

```typescript
try {
  const result = await runUntrustedCode({
    code: 'throw new Error("Oops!")'
  });

  if (!result.success) {
    console.error('Execution failed:', result.error);
  }
} catch (error) {
  console.error('Lambda invocation failed:', error);
}
```

### Batch Execution

```typescript
const results = await Promise.all([
  runUntrustedCode({ code: '1 + 1' }),
  runUntrustedCode({ code: '2 * 2' }),
  runUntrustedCode({ code: '3 ** 3' })
]);

results.forEach(r => console.log(r.result));
// 2, 4, 27
```

### Data Transformation

```typescript
const result = await runUntrustedCode({
  code: `
    const data = [
      { name: 'Alice', score: 85 },
      { name: 'Bob', score: 92 },
      { name: 'Charlie', score: 78 }
    ];

    data.filter(s => s.score >= 80)
        .map(s => ({ ...s, grade: 'A' }))
  `
});

console.log(result.result);
// [{ name: 'Alice', score: 85, grade: 'A' }, { name: 'Bob', score: 92, grade: 'A' }]
```

### Network Access (Fetch API)

The sandbox supports `fetch()` with domain allowlisting for secure network access:

```typescript
// Basic fetch example
const result = await runUntrustedCode({
  code: `
    const response = fetch('https://api.github.com/users/github');
    response.ok ? response.json : { error: response.error }
  `,
  allowedDomains: ['api.github.com']
});

console.log(result.result); // { login: 'github', ... }
```

Advanced example:

```typescript
const result = await runUntrustedCode({
  code: `
    const response = fetch('https://httpbin.org/get');
    if (!response.ok) {
      ({ error: response.error })
    } else {
      ({ status: response.status, data: response.json })
    }
  `,
  allowedDomains: ['httpbin.org']
});
```

**Security Features:**
- Domain allowlisting (exact match or subdomain)
- Private IP blocking (localhost, 127.x.x.x, 10.x.x.x, 192.168.x.x, etc.)
- 5-second timeout per request
- GET requests only (simple and secure)
- Default: No network access

**Fetch Response Object:**
```javascript
{
  ok: boolean,        // true if status 200-299
  status: number,     // HTTP status code
  text: string,       // Response body as text
  json: any,          // Parsed JSON (or null if invalid)
  error?: string      // Error message (if request failed)
}
```

## API Reference

### `runUntrustedCode(options: RunUntrustedCodeOptions): Promise<ExecuteResponse>`

Execute untrusted JavaScript code in a secure AWS Lambda sandbox.

### `RunUntrustedCodeOptions`

```typescript
interface RunUntrustedCodeOptions {
  code: string;              // JavaScript code to execute
  functionName?: string;     // AWS Lambda function name (defaults to SANDBOX_FUNCTION_NAME env var)
  region?: string;           // AWS region (auto-detected from AWS SDK)
  timeoutMs?: number;        // Default: 5000, Max: 25000
  memoryLimitBytes?: number; // Default: 10MB, Max: 50MB
  allowedDomains?: string[]; // Default: [] (no network access)
}
```

### `ExecuteResponse`

```typescript
interface ExecuteResponse {
  success: boolean;
  result?: any;
  error?: string;
  executionTimeMs: number;
  consoleOutput: string[];
}
```

## Environment Variables

- `SANDBOX_FUNCTION_NAME` - Default Lambda function name (required if not specified in options)
- AWS SDK automatically picks up region from:
  - `AWS_REGION` environment variable
  - AWS credentials file
  - EC2 instance metadata
  - ECS container metadata

## Security

The Lambda sandbox provides:
- Memory limits (configurable, max 50MB)
- Timeout protection (configurable, max 25s)
- Removed dangerous globals (require, import, process, etc.)
- Network access control via domain allowlisting
  - Private IP blocking (localhost, 127.x.x.x, 10.x.x.x, 192.168.x.x, etc.)
  - Per-request timeout (5 seconds)
  - Optional subdomain matching
- Console output capture
- QuickJS sandboxing

## License

MIT
