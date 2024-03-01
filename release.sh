
version=$(git for-each-ref --sort=creatordate --format '%(refname)' refs/tags | tail -n 1 | awk -F'/' '{print $3}')
IFS='.' read -r -a version_parts <<< "${version:1}"

patch=$((version_parts[2] + 1))
new_version="v${version_parts[0]}.${version_parts[1]}.$patch"

git tag -a "$new_version" -m "Release $new_version"
git push origin --tags
