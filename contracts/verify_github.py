import requests
import json
import os
import argparse
import sys

from validation import validate_responses
from validation import persist_contract
from validation import get_contract_json

API_TOKEN = os.environ["GITHUB_TOKEN"]
REMOTE = "github"

parser = argparse.ArgumentParser()
parser.add_argument("--persist", action="store_true")
args = parser.parse_args()


def get_headers():
    return {
        "Authorization": f"bearer {API_TOKEN}",
        "Accept": "application/vnd.github.v3+json",
        "X-GitHub-Api-Version": "2022-11-28",
    }


def fake_user(data):
    data["id"] = 123456
    data["node_id"] = "abcdefg"
    data["avatar_url"] = "https://any_url_test.test"


def merge_request_api():
    mr_base_url = "https://api.github.com/repos/jordilin/githapi/pulls"
    mr_existing_url = f"{mr_base_url}/23"
    headers = get_headers()
    response = requests.get(mr_existing_url, headers=headers)
    assert response.status_code == 200
    data = response.json()
    fake_user_data(data)
    if args.persist:
        persist_contract("merge_request.json", REMOTE, data)
    ## open a merge request on existing one - response with a 422
    source_branch = "feature"
    target_branch = "main"
    title = "New Feature"
    body = {
        "title": title,
        "head": source_branch,
        "base": target_branch,
        "body": "This is a new feature",
    }
    response = requests.post(mr_base_url, headers=headers, data=json.dumps(body))
    assert response.status_code == 422
    data_conflict = response.json()
    if args.persist:
        persist_contract("merge_request_conflict.json", REMOTE, data_conflict)
    return data, data_conflict


def fake_user_data(data):
    data["node_id"] = "abcdefg"
    user = data["user"]
    fake_user(user)
    data["id"] = 123456
    user_source = data["head"]["user"]
    fake_user(user_source)
    repo_source = data["head"]["repo"]
    repo_source["id"] = 123456
    repo_source["node_id"] = "abcdefg"
    fake_user(repo_source["owner"])
    user_target = data["base"]["user"]
    fake_user(user_target)
    repo_target = data["base"]["repo"]
    repo_target["id"] = 123456
    repo_target["node_id"] = "abcdefg"
    fake_user(repo_target["owner"])


def get_project_api_json():
    url = "https://api.github.com/repos/jordilin/githapi"
    headers = get_headers()
    response = requests.get(url, headers=headers)
    assert response.status_code == 200
    data = response.json()
    data["id"] = 123456
    data["node_id"] = "abcdefg"
    data["owner"]["id"] = 123456
    data["owner"]["node_id"] = "abcdefg"
    data["owner"]["avatar_url"] = "https://any_url_test.test"
    if args.persist:
        persist_contract("project.json", REMOTE, data)
    return data


def list_pipelines_api():
    url = "https://api.github.com/repos/jordilin/githapi/actions/runs"
    headers = get_headers()
    response = requests.get(url, headers=headers)
    data = response.json()
    run = data["workflow_runs"][0]
    actor = run["actor"]
    fake_user(actor)
    triggering_actor = run["triggering_actor"]
    fake_user(triggering_actor)
    repository_owner = run["repository"]["owner"]
    fake_user(repository_owner)
    head_repository_owner = run["head_repository"]["owner"]
    fake_user(head_repository_owner)
    # patch data with just one workflow run
    data["workflow_runs"] = [run]
    if args.persist:
        persist_contract("list_pipelines.json", REMOTE, data)
    return data


def list_releases_api():
    url = "https://api.github.com/repos/jordilin/githapi/releases"
    headers = get_headers()
    response = requests.get(url, headers=headers)
    data = response.json()
    release = data[0]
    author = release["author"]
    fake_user(author)
    if args.persist:
        persist_contract("list_releases.json", REMOTE, data)
    return data[0]


def get_user_info_api():
    url = "https://api.github.com/user"
    headers = get_headers()
    response = requests.get(url, headers=headers)
    data = response.json()
    if args.persist:
        persist_contract("get_user_info.json", REMOTE, data)
    return data


def list_issues_user_api():
    url = "https://api.github.com/issues"
    headers = get_headers()
    response = requests.get(url, headers=headers)
    data = response.json()
    if args.persist:
        persist_contract("list_issues_user.json", REMOTE, data)
    return data[0]


def list_user_stars_api():
    url = "https://api.github.com/user/starred"
    headers = get_headers()
    response = requests.get(url, headers=headers)
    data = response.json()
    if args.persist:
        persist_contract("stars.json", REMOTE, data)
    return data[0]


class TestAPI:
    def __init__(self, callback, msg, *expected):
        self.callback = callback
        self.msg = msg
        self.expected = expected


if __name__ == "__main__":
    testcases = [
        TestAPI(
            merge_request_api,
            "merge request API contract",
            get_contract_json("merge_request.json", REMOTE),
            get_contract_json("merge_request_conflict.json", REMOTE),
        ),
        TestAPI(
            get_project_api_json,
            "project API contract",
            get_contract_json("project.json", REMOTE),
        ),
        TestAPI(
            list_pipelines_api,
            "list pipelines API contract",
            get_contract_json("list_pipelines.json", REMOTE),
        ),
        TestAPI(
            list_releases_api,
            "list releases API contract",
            get_contract_json("list_releases.json", REMOTE),
        ),
        TestAPI(
            get_user_info_api,
            "get user info API contract",
            get_contract_json("get_user_info.json", REMOTE),
        ),
        TestAPI(
            list_issues_user_api,
            "list issues user API contract",
            get_contract_json("list_issues_user.json", REMOTE),
        ),
        TestAPI(
            list_user_stars_api,
            "list user stars API contract",
            get_contract_json("stars.json", REMOTE),
        ),
    ]
    if not validate_responses(testcases):
        exit(1)
    # TODO
    # # get_project_members_api_json()
