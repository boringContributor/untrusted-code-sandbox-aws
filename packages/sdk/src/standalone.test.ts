import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { runUntrustedCode } from './standalone';
import { LambdaClient, InvokeCommand } from '@aws-sdk/client-lambda';

// Mock the AWS SDK
vi.mock('@aws-sdk/client-lambda', () => ({
  LambdaClient: vi.fn(),
  InvokeCommand: vi.fn(),
}));

describe('runUntrustedCode', () => {
  const mockSend = vi.fn();
  const originalEnv = process.env;

  beforeEach(() => {
    vi.clearAllMocks();
    process.env = { ...originalEnv };

    // Setup Lambda client mock
    (LambdaClient as any).mockImplementation(() => ({
      send: mockSend,
    }));
  });

  afterEach(() => {
    process.env = originalEnv;
  });

  describe('configuration', () => {
    it('should use SANDBOX_FUNCTION_NAME environment variable when functionName not provided', async () => {
      process.env.SANDBOX_FUNCTION_NAME = 'test-function-from-env';

      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: 4,
          executionTimeMs: 10,
          consoleOutput: [],
        })),
      });

      await runUntrustedCode({ code: '2 + 2' });

      expect(InvokeCommand).toHaveBeenCalledWith({
        FunctionName: 'test-function-from-env',
        Payload: expect.any(String),
      });
    });

    it('should use functionName from options over environment variable', async () => {
      process.env.SANDBOX_FUNCTION_NAME = 'env-function';

      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: 4,
          executionTimeMs: 10,
          consoleOutput: [],
        })),
      });

      await runUntrustedCode({
        code: '2 + 2',
        functionName: 'options-function',
      });

      expect(InvokeCommand).toHaveBeenCalledWith({
        FunctionName: 'options-function',
        Payload: expect.any(String),
      });
    });

    it('should throw error when no function name provided', async () => {
      delete process.env.SANDBOX_FUNCTION_NAME;

      await expect(
        runUntrustedCode({ code: '2 + 2' })
      ).rejects.toThrow(
        'Function name must be provided via options.functionName or SANDBOX_FUNCTION_NAME environment variable'
      );
    });

    it('should create Lambda client with region from options', async () => {
      process.env.SANDBOX_FUNCTION_NAME = 'test-function';

      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: 4,
          executionTimeMs: 10,
          consoleOutput: [],
        })),
      });

      await runUntrustedCode({
        code: '2 + 2',
        region: 'us-west-2',
      });

      expect(LambdaClient).toHaveBeenCalledWith({ region: 'us-west-2' });
    });

    it('should create Lambda client with default config when no region specified', async () => {
      process.env.SANDBOX_FUNCTION_NAME = 'test-function';

      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: 4,
          executionTimeMs: 10,
          consoleOutput: [],
        })),
      });

      await runUntrustedCode({ code: '2 + 2' });

      expect(LambdaClient).toHaveBeenCalledWith({});
    });
  });

  describe('code execution', () => {
    beforeEach(() => {
      process.env.SANDBOX_FUNCTION_NAME = 'test-function';
    });

    it('should execute simple code successfully', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: 4,
          executionTimeMs: 10,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({ code: '2 + 2' });

      expect(result).toEqual({
        success: true,
        result: 4,
        executionTimeMs: 10,
        consoleOutput: [],
      });
    });

    it('should pass timeout option to Lambda', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: null,
          executionTimeMs: 10,
          consoleOutput: [],
        })),
      });

      await runUntrustedCode({
        code: 'while(true) {}',
        timeoutMs: 1000,
      });

      const payload = JSON.parse(
        (InvokeCommand as any).mock.calls[0][0].Payload
      );
      expect(payload.timeoutMs).toBe(1000);
    });

    it('should pass memory limit option to Lambda', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: null,
          executionTimeMs: 10,
          consoleOutput: [],
        })),
      });

      await runUntrustedCode({
        code: 'const arr = []',
        memoryLimitBytes: 5 * 1024 * 1024,
      });

      const payload = JSON.parse(
        (InvokeCommand as any).mock.calls[0][0].Payload
      );
      expect(payload.memoryLimitBytes).toBe(5 * 1024 * 1024);
    });

    it('should pass allowed domains to Lambda', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: { data: 'test' },
          executionTimeMs: 100,
          consoleOutput: [],
        })),
      });

      await runUntrustedCode({
        code: 'fetch("https://api.example.com")',
        allowedDomains: ['api.example.com'],
      });

      const payload = JSON.parse(
        (InvokeCommand as any).mock.calls[0][0].Payload
      );
      expect(payload.allowedDomains).toEqual(['api.example.com']);
    });

    it('should pass options parameter to Lambda', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: { userId: '123', name: 'John' },
          executionTimeMs: 50,
          consoleOutput: [],
        })),
      });

      await runUntrustedCode({
        code: 'return { userId: options.userId, name: options.name }',
        options: { userId: '123', name: 'John' },
      });

      const payload = JSON.parse(
        (InvokeCommand as any).mock.calls[0][0].Payload
      );
      expect(payload.options).toEqual({ userId: '123', name: 'John' });
    });

    it('should handle execution errors gracefully', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: 'ReferenceError: undefined is not defined',
          executionTimeMs: 5,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: 'throw new Error("test")',
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('ReferenceError');
    });

    it('should capture console output', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: 42,
          executionTimeMs: 8,
          consoleOutput: ['[log] Hello', '[log] World'],
        })),
      });

      const result = await runUntrustedCode({
        code: 'console.log("Hello"); console.log("World"); 42',
      });

      expect(result.consoleOutput).toEqual(['[log] Hello', '[log] World']);
    });
  });

  describe('error handling', () => {
    beforeEach(() => {
      process.env.SANDBOX_FUNCTION_NAME = 'test-function';
    });

    it('should throw error when Lambda returns no payload', async () => {
      mockSend.mockResolvedValueOnce({});

      await expect(
        runUntrustedCode({ code: '2 + 2' })
      ).rejects.toThrow('No payload in Lambda response');
    });

    it('should throw error when Lambda returns FunctionError', async () => {
      mockSend.mockResolvedValueOnce({
        FunctionError: 'Unhandled',
        Payload: new TextEncoder().encode(JSON.stringify({
          errorMessage: 'Lambda function error',
        })),
      });

      await expect(
        runUntrustedCode({ code: '2 + 2' })
      ).rejects.toThrow('Lambda error: Unhandled');
    });

    it('should throw error when Lambda invocation fails', async () => {
      mockSend.mockRejectedValueOnce(new Error('Network error'));

      await expect(
        runUntrustedCode({ code: '2 + 2' })
      ).rejects.toThrow('Failed to invoke Lambda: Network error');
    });

    it('should handle non-Error exceptions', async () => {
      mockSend.mockRejectedValueOnce('String error');

      await expect(
        runUntrustedCode({ code: '2 + 2' })
      ).rejects.toBe('String error');
    });
  });

  describe('data transformation', () => {
    beforeEach(() => {
      process.env.SANDBOX_FUNCTION_NAME = 'test-function';
    });

    it('should handle array transformations', async () => {
      const result = [
        { name: 'Alice', score: 85, grade: 'A' },
        { name: 'Bob', score: 92, grade: 'A' },
      ];

      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result,
          executionTimeMs: 12,
          consoleOutput: [],
        })),
      });

      const response = await runUntrustedCode({
        code: `
          const data = [
            { name: 'Alice', score: 85 },
            { name: 'Bob', score: 92 },
            { name: 'Charlie', score: 78 }
          ];
          data.filter(s => s.score >= 80).map(s => ({ ...s, grade: 'A' }))
        `,
      });

      expect(response.result).toEqual(result);
    });

    it('should handle complex object results', async () => {
      const complexResult = {
        users: [{ id: 1, name: 'John' }],
        meta: { count: 1, page: 1 },
      };

      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: complexResult,
          executionTimeMs: 15,
          consoleOutput: [],
        })),
      });

      const response = await runUntrustedCode({
        code: 'const data = { users: [{ id: 1, name: "John" }], meta: { count: 1, page: 1 } }; data',
      });

      expect(response.result).toEqual(complexResult);
    });
  });

  describe('unhandled exceptions and runtime errors', () => {
    beforeEach(() => {
      process.env.SANDBOX_FUNCTION_NAME = 'test-function';
    });

    it('should handle network call failures (fetch errors)', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: 'TypeError: fetch failed',
          executionTimeMs: 50,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          const response = await fetch('https://invalid-domain-that-does-not-exist.com');
          return response.json();
        `,
        allowedDomains: ['invalid-domain-that-does-not-exist.com'],
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('TypeError');
      expect(result.error).toContain('fetch');
    });

    it('should handle accessing properties on undefined', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: "TypeError: Cannot read properties of undefined (reading 'name')",
          executionTimeMs: 3,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          const obj = undefined;
          return obj.name;
        `,
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('TypeError');
      expect(result.error).toContain('Cannot read properties of undefined');
    });

    it('should handle accessing properties on null', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: "TypeError: Cannot read properties of null (reading 'value')",
          executionTimeMs: 3,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          const data = null;
          return data.value;
        `,
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('TypeError');
      expect(result.error).toContain('Cannot read properties of null');
    });

    it('should handle network timeout errors', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: 'Error: Request timeout',
          executionTimeMs: 5000,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          const response = await fetch('https://httpstat.us/200?sleep=10000');
          return response.json();
        `,
        timeoutMs: 5000,
        allowedDomains: ['httpstat.us'],
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('timeout');
    });

    it('should handle HTTP error responses (404, 500, etc)', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: 'Error: HTTP 404: Not Found',
          executionTimeMs: 100,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          const response = await fetch('https://api.example.com/not-found');
          if (!response.ok) {
            throw new Error(\`HTTP \${response.status}: \${response.statusText}\`);
          }
          return response.json();
        `,
        allowedDomains: ['api.example.com'],
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('HTTP 404');
    });

    it('should handle JSON parsing errors', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: 'SyntaxError: Unexpected token < in JSON at position 0',
          executionTimeMs: 50,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          const response = await fetch('https://example.com');
          return response.json(); // But response is HTML, not JSON
        `,
        allowedDomains: ['example.com'],
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('SyntaxError');
      expect(result.error).toContain('JSON');
    });

    it('should handle accessing nested properties that do not exist', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: "TypeError: Cannot read properties of undefined (reading 'nested')",
          executionTimeMs: 3,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          const data = { user: { name: 'John' } };
          return data.user.profile.nested.value;
        `,
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('TypeError');
      expect(result.error).toContain('Cannot read properties of undefined');
    });

    it('should handle ReferenceError for undefined variables', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: 'ReferenceError: undefinedVariable is not defined',
          executionTimeMs: 2,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: 'return undefinedVariable;',
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('ReferenceError');
      expect(result.error).toContain('is not defined');
    });

    it('should handle calling undefined as a function', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: 'TypeError: notAFunction is not a function',
          executionTimeMs: 3,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          const notAFunction = 'string';
          return notAFunction();
        `,
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('TypeError');
      expect(result.error).toContain('is not a function');
    });

    it('should handle async errors in promise chains', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: 'Error: Failed in async chain',
          executionTimeMs: 20,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          const data = await Promise.resolve({ items: [] })
            .then(d => d.items.find(i => i.id === 1))
            .then(item => item.name); // item is undefined
          return data;
        `,
      });

      expect(result.success).toBe(false);
      expect(result.error).toBeDefined();
    });

    it('should handle network errors with disallowed domains', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: 'Error: Domain not allowed: blocked-domain.com',
          executionTimeMs: 5,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          const response = await fetch('https://blocked-domain.com/api');
          return response.json();
        `,
        allowedDomains: ['allowed-domain.com'],
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('Domain not allowed');
    });

    it('should handle array access out of bounds gracefully', async () => {
      // JavaScript returns undefined for out of bounds, but accessing properties on it fails
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: "TypeError: Cannot read properties of undefined (reading 'id')",
          executionTimeMs: 3,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          const arr = [1, 2, 3];
          return arr[100].id;
        `,
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('TypeError');
    });

    it('should handle unhandled promise rejections', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: 'Error: Promise rejected',
          executionTimeMs: 10,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          await Promise.reject(new Error('Promise rejected'));
        `,
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('Promise rejected');
    });

    it('should handle division by zero (returns Infinity in JS)', async () => {
      // This is actually not an error in JavaScript, but testing the behavior
      // Note: JSON.stringify converts Infinity to null
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: null, // Infinity becomes null when serialized to JSON
          executionTimeMs: 2,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: 'return 1 / 0;',
      });

      expect(result.success).toBe(true);
      // Infinity gets serialized to null in JSON
      expect(result.result).toBe(null);
    });

    it('should handle malformed response from network call', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: false,
          error: 'SyntaxError: Unexpected end of JSON input',
          executionTimeMs: 60,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          const response = await fetch('https://api.example.com/broken');
          const text = await response.text();
          return JSON.parse(text);
        `,
        allowedDomains: ['api.example.com'],
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('SyntaxError');
      expect(result.error).toContain('JSON');
    });
  });

  describe('integration scenarios', () => {
    beforeEach(() => {
      process.env.SANDBOX_FUNCTION_NAME = 'test-function';
    });

    it('should handle batch execution with Promise.all', async () => {
      mockSend
        .mockResolvedValueOnce({
          Payload: new TextEncoder().encode(JSON.stringify({
            success: true,
            result: 2,
            executionTimeMs: 5,
            consoleOutput: [],
          })),
        })
        .mockResolvedValueOnce({
          Payload: new TextEncoder().encode(JSON.stringify({
            success: true,
            result: 4,
            executionTimeMs: 5,
            consoleOutput: [],
          })),
        })
        .mockResolvedValueOnce({
          Payload: new TextEncoder().encode(JSON.stringify({
            success: true,
            result: 27,
            executionTimeMs: 5,
            consoleOutput: [],
          })),
        });

      const results = await Promise.all([
        runUntrustedCode({ code: '1 + 1' }),
        runUntrustedCode({ code: '2 * 2' }),
        runUntrustedCode({ code: '3 ** 3' }),
      ]);

      expect(results.map(r => r.result)).toEqual([2, 4, 27]);
    });

    it('should handle network access with allowed domains', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: { login: 'github', id: 1 },
          executionTimeMs: 250,
          consoleOutput: [],
        })),
      });

      const result = await runUntrustedCode({
        code: `
          const response = fetch('https://api.github.com/users/github');
          response.ok ? response.json : { error: response.error }
        `,
        allowedDomains: ['api.github.com'],
      });

      expect(result.result).toEqual({ login: 'github', id: 1 });
    });

    it('should work with all options combined', async () => {
      mockSend.mockResolvedValueOnce({
        Payload: new TextEncoder().encode(JSON.stringify({
          success: true,
          result: { data: 'processed' },
          executionTimeMs: 100,
          consoleOutput: ['[log] Processing...'],
        })),
      });

      const result = await runUntrustedCode({
        code: 'console.log("Processing..."); ({ data: "processed" })',
        functionName: 'custom-function',
        region: 'eu-west-1',
        timeoutMs: 3000,
        memoryLimitBytes: 20 * 1024 * 1024,
        allowedDomains: ['api.example.com'],
      });

      expect(result.success).toBe(true);
      expect(result.result).toEqual({ data: 'processed' });
      expect(result.consoleOutput).toEqual(['[log] Processing...']);

      expect(LambdaClient).toHaveBeenCalledWith({ region: 'eu-west-1' });
      expect(InvokeCommand).toHaveBeenCalledWith({
        FunctionName: 'custom-function',
        Payload: expect.any(String),
      });
    });
  });
});
