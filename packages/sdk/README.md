# @untrusted-code/sdk

TypeScript SDK for executing untrusted JavaScript code in AWS Lambda sandbox.

## Installation

```bash
npm install @untrusted-code/sdk
```

## Usage

### Basic Usage

```typescript
import { UntrustedCodeClient } from '@untrusted-code/sdk';

const client = new UntrustedCodeClient({
  functionName: 'your-lambda-function-name',
  region: 'us-east-1'
});

// Execute code
const result = await client.runUntrustedCode({
  code: '2 + 2'
});

console.log(result.result); // 4
console.log(result.success); // true
console.log(result.executionTimeMs); // e.g., 15
```

### Standalone Function

```typescript
import { runUntrustedCode } from '@untrusted-code/sdk';

// Uses LAMBDA_FUNCTION_NAME and AWS_REGION env vars
const result = await runUntrustedCode({
  code: 'console.log("Hello"); return 42;'
});

console.log(result.result); // 42
console.log(result.consoleOutput); // ['[log] Hello']
```

### With Timeout

```typescript
const result = await client.runUntrustedCode({
  code: 'while(true) {}',
  timeoutMs: 1000
});

console.log(result.success); // false
console.log(result.error); // 'Execution timeout exceeded'
```

### Error Handling

```typescript
try {
  const result = await client.runUntrustedCode({
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
const results = await client.batchExecute([
  { code: '1 + 1' },
  { code: '2 * 2' },
  { code: '3 ** 3' }
]);

results.forEach(r => console.log(r.result));
// 2, 4, 27
```

### Data Transformation

```typescript
const result = await client.runUntrustedCode({
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
const result = await client.runUntrustedCode({
  code: `
    const response = fetch('https://api.github.com/users/github');
    response.ok ? response.json : { error: response.error }
  `,
  allowedDomains: ['api.github.com']
});

console.log(result.result); // { login: 'github', ... }
```

Using the helper method:

```typescript
const result = await client.runWithNetworking(
  `
    const response = fetch('https://httpbin.org/get');
    if (!response.ok) {
      ({ error: response.error })
    } else {
      ({ status: response.status, data: response.json })
    }
  `,
  ['httpbin.org']
);
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

### `UntrustedCodeClient`

Constructor options:
- `functionName: string` - AWS Lambda function name or ARN
- `region?: string` - AWS region (optional)
- `lambdaClient?: LambdaClient` - Custom Lambda client (optional)

Methods:
- `runUntrustedCode(request: ExecuteRequest): Promise<ExecuteResponse>` - Execute code
- `runWithTimeout(code: string, timeoutMs: number): Promise<ExecuteResponse>` - Execute with timeout
- `runWithMemoryLimit(code: string, memoryLimitBytes: number): Promise<ExecuteResponse>` - Execute with memory limit
- `runWithNetworking(code: string, allowedDomains: string[]): Promise<ExecuteResponse>` - Execute with network access
- `batchExecute(requests: ExecuteRequest[]): Promise<ExecuteResponse[]>` - Execute multiple codes in parallel

### `ExecuteRequest`

```typescript
interface ExecuteRequest {
  code: string;
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

- `LAMBDA_FUNCTION_NAME` - Default function name for standalone function
- `AWS_REGION` - Default AWS region

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
