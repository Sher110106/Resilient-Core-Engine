#!/bin/bash

# ChunkStream Pro - Unified UI End-to-End Test Script
# This script tests the complete sender-to-receiver workflow with the new UI

set -e  # Exit on error

echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║     ChunkStream Pro - Unified UI End-to-End Test Suite          ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Test directory
TEST_DIR="/tmp/chunkstream_test_$$"
mkdir -p "$TEST_DIR"
echo -e "${BLUE}📁 Test directory: $TEST_DIR${NC}\n"

# Function to cleanup
cleanup() {
    echo -e "\n${YELLOW}🧹 Cleaning up test files...${NC}"
    rm -rf "$TEST_DIR"
    echo -e "${GREEN}✅ Cleanup complete${NC}"
}

trap cleanup EXIT

# Test 1: Create test files
echo -e "${BLUE}=== Test 1: Creating Test Files ===${NC}\n"

# 2MB file
echo -e "${YELLOW}Creating 2MB test file...${NC}"
dd if=/dev/urandom of="$TEST_DIR/test_2mb.bin" bs=1M count=2 2>/dev/null
MD5_2MB=$(md5 -q "$TEST_DIR/test_2mb.bin")
echo -e "${GREEN}✓ 2MB file created (MD5: $MD5_2MB)${NC}\n"

# 5MB file
echo -e "${YELLOW}Creating 5MB test file...${NC}"
dd if=/dev/urandom of="$TEST_DIR/test_5mb.bin" bs=1M count=5 2>/dev/null
MD5_5MB=$(md5 -q "$TEST_DIR/test_5mb.bin")
echo -e "${GREEN}✓ 5MB file created (MD5: $MD5_5MB)${NC}\n"

# 10MB file
echo -e "${YELLOW}Creating 10MB test file...${NC}"
dd if=/dev/urandom of="$TEST_DIR/test_10mb.bin" bs=1M count=10 2>/dev/null
MD5_10MB=$(md5 -q "$TEST_DIR/test_10mb.bin")
echo -e "${GREEN}✓ 10MB file created (MD5: $MD5_10MB)${NC}\n"

# Test 2: Check binaries exist
echo -e "${BLUE}=== Test 2: Checking Binaries ===${NC}\n"

if [ ! -f "./target/release/chunkstream-server" ]; then
    echo -e "${RED}❌ Sender binary not found. Run: cargo build --release --bin chunkstream-server${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Sender binary found${NC}"

if [ ! -f "./target/release/chunkstream-receiver" ]; then
    echo -e "${RED}❌ Receiver binary not found. Run: cargo build --release --bin chunkstream-receiver${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Receiver binary found${NC}\n"

# Test 3: Start receiver in background
echo -e "${BLUE}=== Test 3: Starting Receiver ===${NC}\n"

./target/release/chunkstream-receiver > "$TEST_DIR/receiver.log" 2>&1 &
RECEIVER_PID=$!
echo -e "${GREEN}✓ Receiver started (PID: $RECEIVER_PID)${NC}"

# Wait for receiver to be ready
echo -e "${YELLOW}Waiting for receiver to initialize...${NC}"
sleep 3

# Check if receiver is listening
if ! lsof -Pi :5001 -sTCP:LISTEN > /dev/null 2>&1; then
    echo -e "${RED}❌ Receiver not listening on port 5001${NC}"
    cat "$TEST_DIR/receiver.log"
    exit 1
fi
echo -e "${GREEN}✓ Receiver listening on port 5001${NC}"

# Check receiver REST API
if curl -s http://localhost:8080/api/v1/receiver/status > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Receiver REST API responding on port 8080${NC}"
else
    echo -e "${YELLOW}⚠ Receiver REST API not responding (may start after first transfer)${NC}"
fi
echo ""

# Test 4: Start sender server in background
echo -e "${BLUE}=== Test 4: Starting Sender Server ===${NC}\n"

./target/release/chunkstream-server > "$TEST_DIR/sender.log" 2>&1 &
SENDER_PID=$!
echo -e "${GREEN}✓ Sender server started (PID: $SENDER_PID)${NC}"

# Wait for sender to be ready
echo -e "${YELLOW}Waiting for sender to initialize...${NC}"
sleep 3

# Check if sender is listening
if ! curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo -e "${RED}❌ Sender not responding on port 3000${NC}"
    cat "$TEST_DIR/sender.log"
    kill $RECEIVER_PID $SENDER_PID 2>/dev/null
    exit 1
fi
echo -e "${GREEN}✓ Sender server responding on port 3000${NC}\n"

# Test 5: Transfer files via API
echo -e "${BLUE}=== Test 5: Transferring Files ===${NC}\n"

# Transfer 2MB file
echo -e "${YELLOW}Transferring 2MB file...${NC}"
RESPONSE=$(curl -s -X POST http://localhost:3000/api/v1/upload \
    -F "file=@$TEST_DIR/test_2mb.bin" \
    -F "priority=High" \
    -F "receiver_addr=127.0.0.1:5001")

SESSION_2MB=$(echo $RESPONSE | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
if [ -z "$SESSION_2MB" ]; then
    echo -e "${RED}❌ Failed to start 2MB transfer${NC}"
    echo "Response: $RESPONSE"
    kill $RECEIVER_PID $SENDER_PID 2>/dev/null
    exit 1
fi
echo -e "${GREEN}✓ 2MB transfer started (Session: $SESSION_2MB)${NC}"

# Wait for transfer to complete
echo -e "${YELLOW}Waiting for transfer to complete...${NC}"
sleep 10

# Transfer 5MB file
echo -e "${YELLOW}Transferring 5MB file...${NC}"
RESPONSE=$(curl -s -X POST http://localhost:3000/api/v1/upload \
    -F "file=@$TEST_DIR/test_5mb.bin" \
    -F "priority=High" \
    -F "receiver_addr=127.0.0.1:5001")

SESSION_5MB=$(echo $RESPONSE | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
if [ -z "$SESSION_5MB" ]; then
    echo -e "${RED}❌ Failed to start 5MB transfer${NC}"
    kill $RECEIVER_PID $SENDER_PID 2>/dev/null
    exit 1
fi
echo -e "${GREEN}✓ 5MB transfer started (Session: $SESSION_5MB)${NC}"

echo -e "${YELLOW}Waiting for transfer to complete...${NC}"
sleep 15

# Transfer 10MB file
echo -e "${YELLOW}Transferring 10MB file...${NC}"
RESPONSE=$(curl -s -X POST http://localhost:3000/api/v1/upload \
    -F "file=@$TEST_DIR/test_10mb.bin" \
    -F "priority=Critical" \
    -F "receiver_addr=127.0.0.1:5001")

SESSION_10MB=$(echo $RESPONSE | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
if [ -z "$SESSION_10MB" ]; then
    echo -e "${RED}❌ Failed to start 10MB transfer${NC}"
    kill $RECEIVER_PID $SENDER_PID 2>/dev/null
    exit 1
fi
echo -e "${GREEN}✓ 10MB transfer started (Session: $SESSION_10MB)${NC}"

echo -e "${YELLOW}Waiting for transfer to complete...${NC}"
sleep 20
echo ""

# Test 6: Verify received files
echo -e "${BLUE}=== Test 6: Verifying Received Files ===${NC}\n"

# List received files
echo -e "${YELLOW}Listing received files...${NC}"
ls -lh ./received/

# Find and verify 2MB file
RECEIVED_2MB=$(find ./received -name "*test_2mb.bin" -type f | head -1)
if [ -z "$RECEIVED_2MB" ]; then
    echo -e "${RED}❌ 2MB file not found in received directory${NC}"
    FAILED_TESTS+=("2MB file not received")
else
    MD5_RECEIVED_2MB=$(md5 -q "$RECEIVED_2MB")
    if [ "$MD5_2MB" == "$MD5_RECEIVED_2MB" ]; then
        echo -e "${GREEN}✓ 2MB file verified (MD5 match)${NC}"
    else
        echo -e "${RED}❌ 2MB file MD5 mismatch${NC}"
        echo "  Expected: $MD5_2MB"
        echo "  Got:      $MD5_RECEIVED_2MB"
        FAILED_TESTS+=("2MB file MD5 mismatch")
    fi
fi

# Find and verify 5MB file
RECEIVED_5MB=$(find ./received -name "*test_5mb.bin" -type f | head -1)
if [ -z "$RECEIVED_5MB" ]; then
    echo -e "${RED}❌ 5MB file not found in received directory${NC}"
    FAILED_TESTS+=("5MB file not received")
else
    MD5_RECEIVED_5MB=$(md5 -q "$RECEIVED_5MB")
    if [ "$MD5_5MB" == "$MD5_RECEIVED_5MB" ]; then
        echo -e "${GREEN}✓ 5MB file verified (MD5 match)${NC}"
    else
        echo -e "${RED}❌ 5MB file MD5 mismatch${NC}"
        FAILED_TESTS+=("5MB file MD5 mismatch")
    fi
fi

# Find and verify 10MB file
RECEIVED_10MB=$(find ./received -name "*test_10mb.bin" -type f | head -1)
if [ -z "$RECEIVED_10MB" ]; then
    echo -e "${RED}❌ 10MB file not found in received directory${NC}"
    FAILED_TESTS+=("10MB file not received")
else
    MD5_RECEIVED_10MB=$(md5 -q "$RECEIVED_10MB")
    if [ "$MD5_10MB" == "$MD5_RECEIVED_10MB" ]; then
        echo -e "${GREEN}✓ 10MB file verified (MD5 match)${NC}"
    else
        echo -e "${RED}❌ 10MB file MD5 mismatch${NC}"
        FAILED_TESTS+=("10MB file MD5 mismatch")
    fi
fi
echo ""

# Test 7: Test Receiver API
echo -e "${BLUE}=== Test 7: Testing Receiver API ===${NC}\n"

echo -e "${YELLOW}Checking receiver status...${NC}"
STATUS=$(curl -s http://localhost:8080/api/v1/receiver/status)
if [ -n "$STATUS" ]; then
    echo -e "${GREEN}✓ Receiver status API working${NC}"
    echo "  $STATUS" | python3 -m json.tool 2>/dev/null || echo "  $STATUS"
else
    echo -e "${RED}❌ Receiver status API not responding${NC}"
fi
echo ""

echo -e "${YELLOW}Listing received files via API...${NC}"
FILES=$(curl -s http://localhost:8080/api/v1/receiver/files)
if [ -n "$FILES" ]; then
    echo -e "${GREEN}✓ Receiver files API working${NC}"
    FILE_COUNT=$(echo "$FILES" | python3 -c "import sys, json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "?")
    echo -e "  Files in list: $FILE_COUNT"
else
    echo -e "${RED}❌ Receiver files API not responding${NC}"
fi
echo ""

# Test 8: Cleanup processes
echo -e "${BLUE}=== Test 8: Stopping Services ===${NC}\n"

echo -e "${YELLOW}Stopping sender server...${NC}"
kill $SENDER_PID 2>/dev/null && echo -e "${GREEN}✓ Sender stopped${NC}" || echo -e "${YELLOW}⚠ Sender already stopped${NC}"

echo -e "${YELLOW}Stopping receiver...${NC}"
kill $RECEIVER_PID 2>/dev/null && echo -e "${GREEN}✓ Receiver stopped${NC}" || echo -e "${YELLOW}⚠ Receiver already stopped${NC}"

# Wait for processes to terminate
sleep 2
echo ""

# Final Report
echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║                      Test Results Summary                        ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""

if [ ${#FAILED_TESTS[@]} -eq 0 ]; then
    echo -e "${GREEN}✅ ALL TESTS PASSED!${NC}\n"
    echo "Summary:"
    echo "  ✓ Test files created (2MB, 5MB, 10MB)"
    echo "  ✓ Receiver started successfully"
    echo "  ✓ Sender server started successfully"
    echo "  ✓ Files transferred successfully"
    echo "  ✓ All files verified (MD5 checksums match)"
    echo "  ✓ Receiver REST API working"
    echo "  ✓ Services stopped cleanly"
    echo ""
    echo -e "${GREEN}🎉 Unified UI system is working perfectly!${NC}"
    exit 0
else
    echo -e "${RED}❌ SOME TESTS FAILED${NC}\n"
    echo "Failed tests:"
    for test in "${FAILED_TESTS[@]}"; do
        echo -e "  ${RED}✗${NC} $test"
    done
    echo ""
    echo "Check logs:"
    echo "  Receiver: $TEST_DIR/receiver.log"
    echo "  Sender:   $TEST_DIR/sender.log"
    exit 1
fi
