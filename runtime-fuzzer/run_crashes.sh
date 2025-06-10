#!/bin/sh

# Check if the directory is passed as an argument
if [ $# -eq 0 ]; then
    echo "Usage: $0 <directory>"
    exit 1
fi

# Get the directory from the argument
CRASH_DIR="$1"

# Check if the provided argument is a valid directory
if [ ! -d "$CRASH_DIR" ]; then
    echo "Error: $CRASH_DIR is not a valid directory."
    exit 1
fi

# Iterate over each file in the directory
for file in "$CRASH_DIR"/*; do
    if [ -f "$file" ]; then
        echo "Processing file: $file"
        cargo ziggy run -i "$file"

        # Check if the command succeeded
        if [ $? -ne 0 ]; then
            echo "Error processing $file. Continuing to next file."
        fi
    else
        echo "Skipping non-file item: $file"
    fi
done

echo "All files in $CRASH_DIR processed."
