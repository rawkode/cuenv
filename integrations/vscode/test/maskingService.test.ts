import * as assert from 'assert';
import { maskValue, validateRegexPattern, MaskingService } from '../src/util/masking';

suite('MaskingService Tests', () => {
    suite('validateRegexPattern', () => {
        test('should return true for valid regex patterns', () => {
            assert.strictEqual(validateRegexPattern('test'), true);
            assert.strictEqual(validateRegexPattern('(?i)(secret|token)'), true);
            assert.strictEqual(validateRegexPattern('[A-Z_][A-Z0-9_]*'), true);
        });

        test('should return false for invalid regex patterns', () => {
            assert.strictEqual(validateRegexPattern('['), false);
            assert.strictEqual(validateRegexPattern('*'), false);
            assert.strictEqual(validateRegexPattern('(?'), false);
        });
    });

    suite('maskValue', () => {
        test('should not mask variables that do not match patterns', () => {
            const result = maskValue('normal_var', 'value', ['(?i)(secret|token)']);
            assert.strictEqual(result.shouldMask, false);
            assert.strictEqual(result.masked, 'value');
        });

        test('should mask variables that match patterns', () => {
            const result = maskValue('api_secret', 'my-secret-value', ['(?i)(secret|token)']);
            assert.strictEqual(result.shouldMask, true);
            assert.strictEqual(result.masked, '••••••••');
        });

        test('should handle short values correctly', () => {
            const result = maskValue('secret', 'abc', ['secret']);
            assert.strictEqual(result.shouldMask, true);
            assert.strictEqual(result.masked, '•••');
        });

        test('should handle long values by limiting mask length', () => {
            const result = maskValue('secret', 'very-long-secret-value-that-exceeds-mask-limit', ['secret']);
            assert.strictEqual(result.shouldMask, true);
            assert.strictEqual(result.masked, '••••••••');
        });

        test('should handle invalid regex patterns gracefully', () => {
            const result = maskValue('test_var', 'value', ['[', 'valid_pattern']);
            assert.strictEqual(result.shouldMask, false);
            assert.strictEqual(result.masked, 'value');
        });

        test('should match case-insensitive patterns correctly', () => {
            const result = maskValue('API_SECRET', 'value', ['(?i)secret']);
            assert.strictEqual(result.shouldMask, true);
            assert.strictEqual(result.masked, '•••••');
        });
    });

    suite('MaskingService', () => {
        let maskingService: MaskingService;

        setup(() => {
            maskingService = new MaskingService();
        });

        test('should initially not reveal any variables', () => {
            assert.strictEqual(maskingService.shouldReveal('test'), false);
        });

        test('should toggle reveal state correctly', () => {
            maskingService.toggleReveal('test');
            assert.strictEqual(maskingService.shouldReveal('test'), true);
            
            maskingService.toggleReveal('test');
            assert.strictEqual(maskingService.shouldReveal('test'), false);
        });

        test('should clear all revealed variables', () => {
            maskingService.toggleReveal('test1');
            maskingService.toggleReveal('test2');
            
            assert.strictEqual(maskingService.shouldReveal('test1'), true);
            assert.strictEqual(maskingService.shouldReveal('test2'), true);
            
            maskingService.clearRevealed();
            
            assert.strictEqual(maskingService.shouldReveal('test1'), false);
            assert.strictEqual(maskingService.shouldReveal('test2'), false);
        });

        test('should mask variable correctly when not revealed', () => {
            const result = maskingService.maskVariable('secret', 'my-value', ['secret']);
            assert.strictEqual(result.displayValue, '••••••••');
            assert.strictEqual(result.isRevealed, false);
        });

        test('should show actual value when revealed', () => {
            maskingService.toggleReveal('secret');
            const result = maskingService.maskVariable('secret', 'my-value', ['secret']);
            assert.strictEqual(result.displayValue, 'my-value');
            assert.strictEqual(result.isRevealed, true);
        });

        test('should not mask non-sensitive variables', () => {
            const result = maskingService.maskVariable('normal_var', 'value', ['secret']);
            assert.strictEqual(result.displayValue, 'value');
            assert.strictEqual(result.isRevealed, false);
        });

        test('should preserve masking state across multiple calls', () => {
            maskingService.toggleReveal('secret');
            
            let result = maskingService.maskVariable('secret', 'value1', ['secret']);
            assert.strictEqual(result.isRevealed, true);
            
            result = maskingService.maskVariable('secret', 'value2', ['secret']);
            assert.strictEqual(result.isRevealed, true);
            
            maskingService.toggleReveal('secret');
            result = maskingService.maskVariable('secret', 'value3', ['secret']);
            assert.strictEqual(result.isRevealed, false);
        });
    });
});