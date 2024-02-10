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


def create_merge_request_api():
    url = "https://api.github.com/repos/jordilin/githapi/pulls"
    source_branch = "feature"
    target_branch = "main"
    title = "New Feature"
    headers = {
        "Authorization": f"bearer {API_TOKEN}",
        "Accept": "application/vnd.github.v3+json",
    }
    body = {
        "title": title,
        "head": source_branch,
        "base": target_branch,
        "body": "This is a new feature",
    }
    response = requests.post(url, headers=headers, data=json.dumps(body))
    assert response.status_code == 201
    data = response.json()
    if args.persist:
        persist_contract("merge_request.json", data)
    return data


def get_project_api_json():
    url = "https://api.github.com/repos/jordilin/githapi"
    headers = {
        "Authorization": f"bearer {API_TOKEN}",
        "Accept": "application/vnd.github.v3+json",
    }
    response = requests.get(url, headers=headers)
    assert response.status_code == 200
    data = response.json()
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
            create_merge_request_api,
            "merge request API contract",
            get_contract_json("merge_request.json", REMOTE),
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
