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


def merge_request_api():
    mr_base_url = "https://api.github.com/repos/jordilin/githapi/pulls"
    mr_existing_url = f"{mr_base_url}/23"
    headers = {
        "Authorization": f"bearer {API_TOKEN}",
        "Accept": "application/vnd.github.v3+json",
    }
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
    user["id"] = 123456
    user["avatar_url"] = "https://any_url_test.test"
    user["node_id"] = "abcdefg"
    data["id"] = 123456
    user_source = data["head"]["user"]
    user_source["id"] = 123456
    user_source["avatar_url"] = "https://any_url_test.test"
    user_source["node_id"] = "abcdefg"
    repo_source = data["head"]["repo"]
    repo_source["id"] = 123456
    repo_source["node_id"] = "abcdefg"
    repo_source["owner"]["id"] = 123456
    repo_source["owner"]["node_id"] = "abcdefg"
    user_target = data["base"]["user"]
    user_target["id"] = 123456
    user_target["avatar_url"] = "https://any_url_test.test"
    user_target["node_id"] = "abcdefg"
    repo_target = data["base"]["repo"]
    repo_target["id"] = 123456
    repo_target["node_id"] = "abcdefg"
    repo_target["owner"]["id"] = 123456
    repo_target["owner"]["node_id"] = "abcdefg"


def get_project_api_json():
    url = "https://api.github.com/repos/jordilin/githapi"
    headers = {
        "Authorization": f"bearer {API_TOKEN}",
        "Accept": "application/vnd.github.v3+json",
    }
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
    ]
    if not validate_responses(testcases):
        exit(1)
    # TODO
    # # get_project_members_api_json()
