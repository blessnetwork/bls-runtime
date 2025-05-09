#!/bin/bash
WORkING_DIR="$(mktemp -d)"
cd $WORkING_DIR
# Set the installation directory to the user's bin directory
INSTALL_DIR="$HOME/.blessnet/bin"
GREEN="\033[32m"
BRIGHT_GREEN="\033[92m"
RED="\033[31m"
NC="\033[0m"
# Create the bin directory if it doesn't exist
mkdir -p $INSTALL_DIR

# Determine the operating system and architecture
OS=$(uname -s)
ARCH=$(uname -m)

# Map architectures to download names
case $ARCH in
    "x86_64")
        ARCH_NAME="x86_64"
        ;;
    "aarch64"|"arm64")
        ARCH_NAME="aarch64"
        ;;
    *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

# Check if jq is installed
if ! command -v jq &> /dev/null; then
    echo "jq could not be found. Please install jq to proceed."
    exit 1
fi
# Check if curl is installed
if ! command -v curl &> /dev/null; then
    echo "curl could not be found. Please install curl to proceed."
    exit 1
fi

bls_version=`curl -s https://api.github.com/repos/blessnetwork/bls-runtime/releases/latest|jq -r .tag_name`

# Determine the download URL based on the operating system
case $OS in
    "Linux")
        if [[ "$ARCH" == "x86_64" ]]; then
            URL="https://github.com/blocklessnetwork/bls-runtime/releases/download/${bls_version}/blockless-runtime.linux-latest.x86_64.tar.gz"
        elif [[ "$ARCH" == "aarch64" ]]; then
            URL="https://github.com/blocklessnetwork/bls-runtime/releases/download/${bls_version}/blockless-runtime.linux-latest.aarch64.tar.gz"
        fi
        ;;
    "Darwin")
        if [[ "$ARCH" == "x86_64" ]]; then
            URL="https://github.com/blocklessnetwork/bls-runtime/releases/download/${bls_version}/blockless-runtime.macos-latest.x86_64.tar.gz"
        elif [[ "$ARCH" == "aarch64" || "$ARCH" == "arm64" ]]; then
            URL="https://github.com/blocklessnetwork/bls-runtime/releases/download/${bls_version}/blockless-runtime.macos-latest.aarch64.tar.gz"
        fi
        ;;
    "WindowsNT")
        if [[ "$ARCH" == "x86_64" ]]; then
            URL="https://github.com/blocklessnetwork/bls-runtime/releases/download/${bls_version}/blockless-runtime.windows-latest.x86_64.tar.gz"
        fi
        ;;
    *)
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac

# Download the binary
echo "Downloading and Extracting Blockless Runtime from $URL..."
curl -L $URL | tar -xz 
if [ $? -ne 0 ]; then
    echo "Error downloading Blockless Runtime. Please check your internet connection."
    exit 1
fi
# Check if the download was successful
if [ ! -f bls-runtime ]; then
    echo "Error: Blockless Runtime binary not found in the downloaded archive."
    exit 1
fi

# Move the binary to the user's bin directory
echo "Installing Blockless Runtime to $INSTALL_DIR..."
mv bls-runtime $INSTALL_DIR

# Make sure the binary is executable
chmod +x $INSTALL_DIR/bls-runtime

# Clean up
rm  -rf $WORkING_DIR

# Add bin to PATH if not already added
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo -e "${RED}Please add follow line to your shell profile...${NC}"
    echo -e "${BRIGHT_GREEN}export PATH=$INSTALL_DIR:\$PATH${NC}"
fi

# Verify the installation
echo -e "Install complete!"
$INSTALL_DIR/bls-runtime --version
