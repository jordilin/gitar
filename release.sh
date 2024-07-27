help() {
    echo "Usage: $0 [options] [version]"
    echo
    echo "Options:"
    echo "  -h, --help    Show this help message and exit"
    echo
    echo "If a version is provided, it will be used instead of auto-incrementing the latest tag."
    echo "Version should be in the format X.Y.Z (without the 'v' prefix)."
}

# Check for help flag
if [[ "$1" == "-h" ]] || [[ "$1" == "--help" ]]; then
    help
    exit 0
fi

if [ "$(git rev-parse --abbrev-ref HEAD)" != "main" ]; then
    echo "Not in main branch"
    exit 1
fi

# make sure we are in sync
git pull --prune

if [ -n "$1" ]; then
    # Use user-provided version, removing the 'v' prefix if present
    new_version="${1#v}"
    tag_version="v${new_version}"
else
    # Auto-increment version based on latest tag
    version=$(git for-each-ref --sort=creatordate --format '%(refname)' refs/tags | tail -n 1 | awk -F'/' '{print $3}')
    IFS='.' read -r -a version_parts <<<"${version:1}"

    patch=$((version_parts[2] + 1))
    new_version="${version_parts[0]}.${version_parts[1]}.$patch"
    tag_version="v${new_version}"
fi

cargo_version=$(grep -oP '^version = "\K[^"]+' Cargo.toml)

# If new version not setup in Cargo.toml then bail
if [ "$new_version" != "$cargo_version" ]; then
    echo "Cargo.toml version is not updated. Please update it to $new_version"
    exit 1
fi

git tag -a "$tag_version" -m "Release $tag_version"
git push origin --tags
