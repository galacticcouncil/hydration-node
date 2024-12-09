import os

def find_relevant_lines(root_dir, output_file):
    search_terms = [
        'pause_transaction',
        'set_asset_tradable_state',
        'DispatchClass::Operational'
    ]

    rust_files = []
    for dirpath, dirnames, filenames in os.walk(root_dir):
        if 'target' in dirnames:
            dirnames.remove('target')
        for filename in filenames:
            if filename.endswith('.rs'):
                rust_files.append(os.path.join(dirpath, filename))

    matches_found = []
    for fname in rust_files:
        with open(fname, 'r') as f:
            lines = f.readlines()
        file_matched = False
        matched_lines = []
        for i, line in enumerate(lines):
            if any(term in line for term in search_terms):
                file_matched = True
                # Just record line number without printing full code line
                matched_lines.append(i+1)
        if file_matched:
            matches_found.append((fname, matched_lines))

    with open(output_file, 'w') as outfile:
        for fname, lines in matches_found:
            outfile.write(f"File: {fname}\n")
            outfile.write("Matched lines: " + ", ".join(str(l) for l in lines) + "\n\n")

if __name__ == "__main__":
    project_root = './pallets'
    output_path = 'relevant_matches_summary.txt'
    find_relevant_lines(project_root, output_path)
