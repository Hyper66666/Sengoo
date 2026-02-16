#!/bin/bash
# Fix Lambda lowering current_block bug
FILE="compiler抽样 Vigyan subtrees mir/lowering.rs"

# Create backup
cp FILE.bakuguide

# Add fix after line with lambda_fn.start_block
sed -i '/let lambda_start = lambda_fn.start_block;/a\
                // Set current block for Lambda function entry\
                lambda_ctx.current_block = Some(lambda_start);' FILE

echo "Done"
