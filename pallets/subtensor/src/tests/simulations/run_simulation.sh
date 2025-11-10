#!/bin/bash
# Automated TAO Flow Merger Simulation Runner
#
# This script:
# 1. Sets up Python virtual environment using uv
# 2. Runs the Rust simulation tests
# 3. Captures output to file
# 4. Automatically generates graphs
# 5. Opens results

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../../../.." && pwd)"
OUTPUT_FILE="/tmp/merger_flow_output.txt"
GRAPH_DIR="$PROJECT_ROOT/merger_flow_results"
VENV_DIR="$SCRIPT_DIR/.venv"

echo -e "${BLUE}╔═══════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   TAO Flow Merger Simulation - Automated Runner                   ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check for uv, install if needed
echo -e "${YELLOW}Checking for uv...${NC}"
if ! command -v uv &> /dev/null; then
    echo -e "${YELLOW}uv not found. Installing uv...${NC}"
    curl -LsSf https://astral.sh/uv/install.sh | sh

    # Add uv to PATH for this session
    export PATH="$HOME/.cargo/bin:$PATH"

    if ! command -v uv &> /dev/null; then
        echo -e "${RED}ERROR: Failed to install uv${NC}"
        echo "Please install manually: https://github.com/astral-sh/uv"
        exit 1
    fi
fi

echo -e "${GREEN}✓ uv found${NC}"

# Create/activate virtual environment
echo -e "${YELLOW}Setting up Python virtual environment...${NC}"
if [ ! -d "$VENV_DIR" ]; then
    echo "  Creating new venv at: $VENV_DIR"
    uv venv "$VENV_DIR"
fi

# Activate venv
source "$VENV_DIR/bin/activate"
echo -e "${GREEN}✓ Virtual environment activated${NC}"

# Install dependencies with uv
echo -e "${YELLOW}Installing Python dependencies (matplotlib, pandas)...${NC}"
uv pip install matplotlib pandas

echo -e "${GREEN}✓ Dependencies installed${NC}"
echo ""

# Run simulation
echo -e "${YELLOW}Running Rust simulation tests...${NC}"
echo "  Output: $OUTPUT_FILE"
echo ""

cd "$PROJECT_ROOT"

SKIP_WASM_BUILD=1 cargo test --package pallet-subtensor --lib \
  -- tests::simulations::merger_flow::test_merger_flow_all_scenarios_combined \
  --nocapture > "$OUTPUT_FILE" 2>&1

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Simulation completed successfully${NC}"
else
    echo -e "${RED}✗ Simulation failed${NC}"
    echo "Check output: $OUTPUT_FILE"
    exit 1
fi

# Generate graphs
echo ""
echo -e "${YELLOW}Generating graphs...${NC}"
echo "  Graphs will be saved to: $GRAPH_DIR"

mkdir -p "$GRAPH_DIR"

python3 "$SCRIPT_DIR/graph_merger_flow.py" "$OUTPUT_FILE" "$GRAPH_DIR"

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Graphs generated successfully${NC}"
else
    echo -e "${RED}✗ Graph generation failed${NC}"
    exit 1
fi

# Summary
echo ""
echo -e "${BLUE}╔═══════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    SIMULATION COMPLETE                             ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${GREEN}Results:${NC}"
echo "  • Simulation output: $OUTPUT_FILE"
echo "  • Graphs directory:  $GRAPH_DIR"
echo ""
echo -e "${GREEN}Generated graphs:${NC}"
ls -1 "$GRAPH_DIR"/*.png 2>/dev/null | while read file; do
    echo "  • $(basename "$file")"
done

echo ""
echo -e "${YELLOW}To view results:${NC}"
echo "  cat $OUTPUT_FILE"
echo "  open $GRAPH_DIR  # macOS"
echo "  xdg-open $GRAPH_DIR  # Linux"
echo ""

# Optionally open results
if command -v open &> /dev/null; then
    read -p "Open graph directory? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        open "$GRAPH_DIR"
    fi
elif command -v xdg-open &> /dev/null; then
    read -p "Open graph directory? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        xdg-open "$GRAPH_DIR"
    fi
fi

# Deactivate virtual environment
deactivate 2>/dev/null || true

echo -e "\n${GREEN}Done!${NC}"
