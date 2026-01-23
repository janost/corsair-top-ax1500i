#!/bin/bash

# corsair-top launcher with splash logo

BINARY="/usr/local/bin/corsair-top"

# Check if binary exists
if [ ! -f "$BINARY" ]; then
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if [ -f "$SCRIPT_DIR/target/release/corsair-top" ]; then
        BINARY="$SCRIPT_DIR/target/release/corsair-top"
    elif [ -f "$SCRIPT_DIR/target/debug/corsair-top" ]; then
        BINARY="$SCRIPT_DIR/target/debug/corsair-top"
    else
        echo "Error: corsair-top binary not found. Run install.sh first."
        exit 1
    fi
fi

# Colors
C_RESET='\033[0m'
C_WHITE='\033[1;37m'
C_CYAN='\033[0;36m'
C_GREEN='\033[0;32m'
C_YELLOW='\033[1;33m'
C_BLUE='\033[0;34m'
C_DIM='\033[2m'

clear

echo ""
echo ""
echo -e "${C_WHITE}           ╱╲${C_RESET}"
echo -e "${C_WHITE}          ╱╱╲╲${C_RESET}"
echo -e "${C_WHITE}         ╱╱╱╲╲╲${C_RESET}"
echo -e "${C_WHITE}        ╱╱╱╱╲╲╲╲${C_RESET}"
echo -e "${C_WHITE}       ╱╱╱╱╱╲╲╲╲╲${C_RESET}"
echo -e "${C_WHITE}      ╱╱╱╱╱╱╲╲╲╲╲╲${C_RESET}"
echo ""
echo -e "${C_GREEN}      ●${C_RESET}"
echo -e "${C_GREEN}       ╲${C_YELLOW}──────●${C_RESET}"
echo -e "${C_YELLOW}               ╲${C_RESET}"
echo -e "${C_YELLOW}                ╲${C_CYAN}────●${C_RESET}"
echo -e "${C_CYAN}                      ╲${C_BLUE}──●${C_RESET}"
echo ""
echo -e "${C_CYAN}        c o r s a i r - t o p${C_RESET}"
echo -e "${C_DIM}       Corsair AX1600i PSU Monitor${C_RESET}"
echo ""
echo ""

sleep 1.5

exec sudo "$BINARY" "$@"
