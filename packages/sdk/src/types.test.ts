import { describe, it, expectTypeOf } from 'vitest';
import type { RunUntrustedCodeOptions, ExecuteResponse } from './types';

describe('Type definitions', () => {
  describe('RunUntrustedCodeOptions', () => {
    it('should require code property', () => {
      expectTypeOf<RunUntrustedCodeOptions>().toHaveProperty('code');
      expectTypeOf<RunUntrustedCodeOptions['code']>().toBeString();
    });

    it('should have optional functionName', () => {
      expectTypeOf<RunUntrustedCodeOptions['functionName']>().toEqualTypeOf<string | undefined>();
    });

    it('should have optional region', () => {
      expectTypeOf<RunUntrustedCodeOptions['region']>().toEqualTypeOf<string | undefined>();
    });

    it('should have optional timeoutMs', () => {
      expectTypeOf<RunUntrustedCodeOptions['timeoutMs']>().toEqualTypeOf<number | undefined>();
    });

    it('should have optional memoryLimitBytes', () => {
      expectTypeOf<RunUntrustedCodeOptions['memoryLimitBytes']>().toEqualTypeOf<number | undefined>();
    });

    it('should have optional allowedDomains', () => {
      expectTypeOf<RunUntrustedCodeOptions['allowedDomains']>().toEqualTypeOf<string[] | undefined>();
    });

    it('should accept minimal valid options', () => {
      const minimalOptions: RunUntrustedCodeOptions = {
        code: '2 + 2',
      };
      expectTypeOf(minimalOptions).toMatchTypeOf<RunUntrustedCodeOptions>();
    });

    it('should accept all options', () => {
      const fullOptions: RunUntrustedCodeOptions = {
        code: '2 + 2',
        functionName: 'test-function',
        region: 'us-east-1',
        timeoutMs: 5000,
        memoryLimitBytes: 10485760,
        allowedDomains: ['api.example.com'],
      };
      expectTypeOf(fullOptions).toMatchTypeOf<RunUntrustedCodeOptions>();
    });
  });

  describe('ExecuteResponse', () => {
    it('should require success property', () => {
      expectTypeOf<ExecuteResponse>().toHaveProperty('success');
      expectTypeOf<ExecuteResponse['success']>().toBeBoolean();
    });

    it('should have optional result', () => {
      expectTypeOf<ExecuteResponse['result']>().toEqualTypeOf<any>();
    });

    it('should have optional error', () => {
      expectTypeOf<ExecuteResponse['error']>().toEqualTypeOf<string | undefined>();
    });

    it('should require executionTimeMs', () => {
      expectTypeOf<ExecuteResponse>().toHaveProperty('executionTimeMs');
      expectTypeOf<ExecuteResponse['executionTimeMs']>().toBeNumber();
    });

    it('should require consoleOutput', () => {
      expectTypeOf<ExecuteResponse>().toHaveProperty('consoleOutput');
      expectTypeOf<ExecuteResponse['consoleOutput']>().toEqualTypeOf<string[]>();
    });

    it('should accept successful response', () => {
      const successResponse: ExecuteResponse = {
        success: true,
        result: 42,
        executionTimeMs: 10,
        consoleOutput: ['[log] test'],
      };
      expectTypeOf(successResponse).toMatchTypeOf<ExecuteResponse>();
    });

    it('should accept error response', () => {
      const errorResponse: ExecuteResponse = {
        success: false,
        error: 'Execution failed',
        executionTimeMs: 5,
        consoleOutput: [],
      };
      expectTypeOf(errorResponse).toMatchTypeOf<ExecuteResponse>();
    });

    it('should allow any result type', () => {
      const stringResult: ExecuteResponse = {
        success: true,
        result: 'hello',
        executionTimeMs: 10,
        consoleOutput: [],
      };

      const objectResult: ExecuteResponse = {
        success: true,
        result: { foo: 'bar' },
        executionTimeMs: 10,
        consoleOutput: [],
      };

      const arrayResult: ExecuteResponse = {
        success: true,
        result: [1, 2, 3],
        executionTimeMs: 10,
        consoleOutput: [],
      };

      expectTypeOf(stringResult).toMatchTypeOf<ExecuteResponse>();
      expectTypeOf(objectResult).toMatchTypeOf<ExecuteResponse>();
      expectTypeOf(arrayResult).toMatchTypeOf<ExecuteResponse>();
    });
  });
});
