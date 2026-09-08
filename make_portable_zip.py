import zipfile, os

EXCLUDE_DIRS = {'src-tauri/target', 'src-tauri/whisper-build', 'src-tauri/gen',
                'node_modules', 'release', 'dist', '.video_agent',
                'docs/manual_build'}
EXCLUDE_EXT = {'.png'}
out = '../BibleHub-dev-portable.zip'
if os.path.exists(out):
    os.remove(out)

count = 0
z = zipfile.ZipFile(out, 'w', zipfile.ZIP_DEFLATED)
for root, dirs, files in os.walk('.'):
    rel = os.path.relpath(root, '.').replace(os.sep, '/')
    dirs[:] = [d for d in dirs
               if (f"{rel}/{d}" if rel != '.' else d) not in EXCLUDE_DIRS
               and d not in {'.git'}]
    for f in files:
        if os.path.splitext(f)[1].lower() in EXCLUDE_EXT:
            continue
        rel_path = f"{rel}/{f}" if rel != '.' else f
        if f.endswith('.db') or f.endswith('.zip') or f.endswith('.bin'):
            continue
        z.write(os.path.join(root, f), rel_path)
        count += 1
z.close()
print(f"{count} files -> {out} ({os.path.getsize(out)/1048576:.1f} MB)")
