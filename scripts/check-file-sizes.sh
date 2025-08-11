#!/usr/bin/env bash
set -euo pipefail

# Check file sizes for refactoring progress
# Helps ensure no files exceed 200 lines (except test files up to 400 lines)

RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

echo "📊 File Size Analysis for cuenv"
echo "==============================="

# Find all Rust files excluding target directory
rust_files=$(find . -name "*.rs" -type f | grep -v target | sort)

# Counters
over_200=0
over_400=0
test_files_over_400=0
largest_files=()

echo -e "\n🔍 Analyzing Rust files..."

while IFS= read -r file; do
    lines=$(wc -l < "$file")
    
    if [[ $lines -gt 200 ]]; then
        over_200=$((over_200 + 1))
        
        # Check if it's a test file
        if [[ "$file" =~ (tests/|test\.rs|_test\.rs|benches/) ]]; then
            if [[ $lines -gt 400 ]]; then
                test_files_over_400=$((test_files_over_400 + 1))
                echo -e "${RED}⚠️  TEST FILE TOO LARGE: $file ($lines lines)${NC}"
            else
                echo -e "${YELLOW}📝 Test file: $file ($lines lines)${NC}"
            fi
        else
            echo -e "${RED}❌ $file ($lines lines)${NC}"
            if [[ $lines -gt 1000 ]]; then
                largest_files+=("$file:$lines")
            fi
        fi
    fi
done <<< "$rust_files"

echo -e "\n📈 Summary:"
echo "============"
echo -e "Files over 200 lines: ${RED}$over_200${NC}"
echo -e "Test files over 400 lines: ${RED}$test_files_over_400${NC}"

if [[ ${#largest_files[@]} -gt 0 ]]; then
    echo -e "\n🚨 Critical files (1000+ lines):"
    for file_info in "${largest_files[@]}"; do
        file=${file_info%:*}
        lines=${file_info#*:}
        echo -e "${RED}   $file ($lines lines)${NC}"
    done
fi

echo -e "\n🎯 Refactoring Progress:"
total_rust_files=$(echo "$rust_files" | wc -l)
compliant_files=$((total_rust_files - over_200))
progress=$((compliant_files * 100 / total_rust_files))
echo -e "Compliant files: ${GREEN}$compliant_files/$total_rust_files${NC} (${progress}%)"

# Exit with error if there are violations
if [[ $over_200 -gt 0 ]] || [[ $test_files_over_400 -gt 0 ]]; then
    echo -e "\n❌ File size violations found!"
    exit 1
else
    echo -e "\n✅ All files comply with size limits!"
    exit 0
fi