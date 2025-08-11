import { execFile } from 'child_process';
import { promisify } from 'util';
import * as path from 'path';
import { EnvironmentJson } from '../types/environment';
import { TaskJson, TaskDefinition } from '../types/task';

const execFileAsync = promisify(execFile);

export interface CLIResult {
    stdout: string;
    stderr: string;
}

export class CLIAdapter {
    private executablePath: string;
    private timeout: number;

    constructor(executablePath: string = 'cuenv', timeout: number = 5000) {
        this.executablePath = executablePath;
        this.timeout = timeout;
    }

    private sanitizeArgs(args: string[]): string[] {
        // Basic sanitization - remove null bytes and control characters
        return args.map(arg => 
            arg.replace(/[\x00-\x1f\x7f-\x9f]/g, '')
               .trim()
        ).filter(arg => arg.length > 0);
    }

    private async execCuenv(args: string[], cwd: string): Promise<CLIResult> {
        const sanitizedArgs = this.sanitizeArgs(args);
        try {
            const result = await execFileAsync(this.executablePath, sanitizedArgs, {
                cwd,
                timeout: this.timeout,
                encoding: 'utf8'
            });
            return { stdout: result.stdout, stderr: result.stderr };
        } catch (error: any) {
            // Handle different error types
            if (error.code === 'ENOENT') {
                throw new Error(`cuenv binary not found at path: ${this.executablePath}`);
            }
            if (error.code === 'TIMEOUT') {
                throw new Error(`cuenv command timed out after ${this.timeout}ms`);
            }
            throw new Error(`cuenv command failed: ${error.message}\nstderr: ${error.stderr}`);
        }
    }

    async exportEnv(workspaceFolder: string): Promise<EnvironmentJson> {
        // For now, parse shell export format since --json isn't available for export yet
        const result = await this.execCuenv(['env', 'export'], workspaceFolder);
        return this.parseShellExports(result.stdout);
    }

    private parseShellExports(shellOutput: string): EnvironmentJson {
        const variables: Record<string, string> = {};
        const lines = shellOutput.split('\n');
        
        for (const line of lines) {
            const trimmed = line.trim();
            if (trimmed.startsWith('export ')) {
                // Parse: export VAR_NAME="value" or export VAR_NAME=value
                const match = trimmed.match(/^export\s+([A-Za-z_][A-Za-z0-9_]*)=(.*)$/i);
                if (match) {
                    const [, name, value] = match;
                    // Remove quotes if present
                    let cleanValue = value;
                    if ((value.startsWith('"') && value.endsWith('"')) || 
                        (value.startsWith("'") && value.endsWith("'"))) {
                        cleanValue = value.slice(1, -1);
                    }
                    variables[name] = cleanValue;
                }
            }
        }
        
        return { variables };
    }

    async listTasks(workspaceFolder: string): Promise<TaskDefinition[]> {
        try {
            // Use TSP export-json command
            const result = await this.execCuenv(['internal', 'task-protocol', '--export-json'], workspaceFolder);
            const taskJson: TaskJson = JSON.parse(result.stdout);
            return taskJson.tasks || [];
        } catch (error) {
            if (error instanceof Error && error.message.includes('JSON')) {
                throw new Error('Failed to parse task JSON from cuenv. Make sure you have the latest version.');
            }
            throw error;
        }
    }

    async runTask(workspaceFolder: string, taskName: string): Promise<void> {
        // Use TSP run-task command for programmatic execution
        await this.execCuenv(['internal', 'task-protocol', '--run-task', taskName], workspaceFolder);
    }

    async checkBinaryExists(): Promise<boolean> {
        try {
            await execFileAsync(this.executablePath, ['--version'], { timeout: 2000 });
            return true;
        } catch {
            return false;
        }
    }

    updateExecutablePath(newPath: string): void {
        this.executablePath = newPath;
    }
}