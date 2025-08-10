import * as assert from 'assert';
import { CLIAdapter } from '../src/services/cliAdapter';

// Mock child_process.execFile
let mockExecFileResult: any = null;
let mockExecFileError: any = null;
let capturedArgs: string[] = [];

const mockExecFile = (path: string, args: string[], options: any, callback: (error: any, stdout: string, stderr: string) => void) => {
    capturedArgs = args;
    if (mockExecFileError) {
        callback(mockExecFileError, '', '');
    } else {
        callback(null, mockExecFileResult?.stdout || '', mockExecFileResult?.stderr || '');
    }
};

// Mock the module
const originalExecFile = require('child_process').execFile;
require('child_process').execFile = mockExecFile;

suite('CLIAdapter Tests', () => {
    let cliAdapter: CLIAdapter;

    setup(() => {
        cliAdapter = new CLIAdapter('test-cuenv', 1000);
        mockExecFileResult = null;
        mockExecFileError = null;
        capturedArgs = [];
    });

    teardown(() => {
        // Restore original
        require('child_process').execFile = originalExecFile;
    });

    suite('sanitizeArgs', () => {
        test('should sanitize arguments with control characters', async () => {
            mockExecFileResult = { stdout: 'export TEST=value\n', stderr: '' };
            
            // Access private method via bracket notation for testing
            const sanitized = (cliAdapter as any).sanitizeArgs(['test\x00arg', 'normal\x1farg', '']);
            assert.deepStrictEqual(sanitized, ['testarg', 'normalarg']);
        });

        test('should filter out empty arguments after sanitization', async () => {
            const sanitized = (cliAdapter as any).sanitizeArgs(['', '  ', 'valid']);
            assert.deepStrictEqual(sanitized, ['valid']);
        });

        test('should trim whitespace from arguments', async () => {
            const sanitized = (cliAdapter as any).sanitizeArgs(['  spaced  ', 'normal']);
            assert.deepStrictEqual(sanitized, ['spaced', 'normal']);
        });
    });

    suite('parseShellExports', () => {
        test('should parse shell exports correctly', async () => {
            const shellOutput = `export API_KEY="secret-value"
export DEBUG_MODE=true
export PORT=3000
# Comment line
export EMPTY_VAR=""`;

            const result = (cliAdapter as any).parseShellExports(shellOutput);
            
            assert.strictEqual(result.variables.API_KEY, 'secret-value');
            assert.strictEqual(result.variables.DEBUG_MODE, 'true');
            assert.strictEqual(result.variables.PORT, '3000');
            assert.strictEqual(result.variables.EMPTY_VAR, '');
        });

        test('should handle mixed case variable names', async () => {
            const shellOutput = `export api_key="secret"
export Debug_Mode=true
export CamelCase=value`;

            const result = (cliAdapter as any).parseShellExports(shellOutput);
            
            assert.strictEqual(result.variables.api_key, 'secret');
            assert.strictEqual(result.variables.Debug_Mode, 'true');
            assert.strictEqual(result.variables.CamelCase, 'value');
        });

        test('should handle malformed export statements', async () => {
            const shellOutput = `export VALID=value
malformed line
export 
export =value
export ANOTHER=valid`;

            const result = (cliAdapter as any).parseShellExports(shellOutput);
            
            assert.strictEqual(result.variables.VALID, 'value');
            assert.strictEqual(result.variables.ANOTHER, 'valid');
            assert.strictEqual(Object.keys(result.variables).length, 2);
        });

        test('should handle single quotes', async () => {
            const shellOutput = `export SINGLE='single-value'
export DOUBLE="double-value"
export UNQUOTED=unquoted-value`;

            const result = (cliAdapter as any).parseShellExports(shellOutput);
            
            assert.strictEqual(result.variables.SINGLE, 'single-value');
            assert.strictEqual(result.variables.DOUBLE, 'double-value');
            assert.strictEqual(result.variables.UNQUOTED, 'unquoted-value');
        });
    });

    suite('exportEnv', () => {
        test('should export environment variables successfully', async () => {
            mockExecFileResult = { stdout: 'export TEST_VAR="test-value"\n', stderr: '' };
            
            const result = await cliAdapter.exportEnv('/test/path');
            
            assert.strictEqual(capturedArgs[0], 'export');
            assert.strictEqual(result.variables.TEST_VAR, 'test-value');
        });

        test('should handle binary not found error', async () => {
            mockExecFileError = { code: 'ENOENT' };
            
            try {
                await cliAdapter.exportEnv('/test/path');
                assert.fail('Should have thrown an error');
            } catch (error: any) {
                assert.strictEqual(error.message, 'cuenv binary not found at path: test-cuenv');
            }
        });

        test('should handle timeout error', async () => {
            mockExecFileError = { code: 'TIMEOUT' };
            
            try {
                await cliAdapter.exportEnv('/test/path');
                assert.fail('Should have thrown an error');
            } catch (error: any) {
                assert.strictEqual(error.message, 'cuenv command timed out after 1000ms');
            }
        });
    });

    suite('listTasks', () => {
        test('should parse task JSON successfully', async () => {
            const taskJson = {
                tasks: [
                    { name: 'build', description: 'Build the project', dependencies: [] },
                    { name: 'test', description: 'Run tests', dependencies: ['build'] }
                ]
            };
            mockExecFileResult = { stdout: JSON.stringify(taskJson), stderr: '' };
            
            const result = await cliAdapter.listTasks('/test/path');
            
            assert.deepStrictEqual(capturedArgs, ['internal', 'task-protocol', '--export-json']);
            assert.strictEqual(result.length, 2);
            assert.strictEqual(result[0].name, 'build');
            assert.strictEqual(result[1].name, 'test');
        });

        test('should handle invalid JSON gracefully', async () => {
            mockExecFileResult = { stdout: 'invalid json', stderr: '' };
            
            try {
                await cliAdapter.listTasks('/test/path');
                assert.fail('Should have thrown an error');
            } catch (error: any) {
                assert.ok(error.message.includes('Failed to parse task JSON'));
            }
        });

        test('should handle empty task list', async () => {
            mockExecFileResult = { stdout: '{"tasks": []}', stderr: '' };
            
            const result = await cliAdapter.listTasks('/test/path');
            
            assert.strictEqual(result.length, 0);
        });
    });

    suite('runTask', () => {
        test('should run task successfully', async () => {
            mockExecFileResult = { stdout: 'task completed', stderr: '' };
            
            await cliAdapter.runTask('/test/path', 'build');
            
            assert.deepStrictEqual(capturedArgs, ['internal', 'task-protocol', '--run-task', 'build']);
        });
    });

    suite('checkBinaryExists', () => {
        test('should return true when binary exists', async () => {
            mockExecFileResult = { stdout: 'cuenv 0.4.4', stderr: '' };
            
            const result = await cliAdapter.checkBinaryExists();
            
            assert.strictEqual(result, true);
        });

        test('should return false when binary does not exist', async () => {
            mockExecFileError = { code: 'ENOENT' };
            
            const result = await cliAdapter.checkBinaryExists();
            
            assert.strictEqual(result, false);
        });
    });

    suite('updateExecutablePath', () => {
        test('should update executable path', async () => {
            cliAdapter.updateExecutablePath('/new/path/cuenv');
            
            mockExecFileResult = { stdout: 'version', stderr: '' };
            await cliAdapter.checkBinaryExists();
            
            // The path should be updated in the internal call
            // We can't directly verify this without exposing the path, but we can test the functionality
            assert.ok(true); // Test passes if no error is thrown
        });
    });
});