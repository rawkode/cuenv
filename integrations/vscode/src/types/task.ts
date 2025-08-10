import { Event } from 'vscode';

export interface TaskDefinition {
    name: string;
    description?: string;
    after: string[];  // dependencies
}

export interface TaskJson {
    tasks: TaskDefinition[];
}

export interface TaskEvents {
    onDidChangeTasks: Event<TaskDefinition[]>;
    onTaskStarted: Event<string>;
    onTaskFinished: Event<{ name: string; success: boolean }>;
    onTaskError: Event<{ name: string; error: string }>;
}

export interface TaskItem {
    name: string;
    description?: string;
    dependencies: string[];
}