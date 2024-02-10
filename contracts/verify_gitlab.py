import requests
import json
import os
import argparse

from validation import validate_responses
from validation import persist_contract
from validation import get_contract_json

PRIVATE_TOKEN = os.environ["GITLAB_TOKEN"]
REMOTE = "gitlab"

parser = argparse.ArgumentParser()
parser.add_argument("--persist", action="store_true")
args = parser.parse_args()


def get_project_api_json():
    url = "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi"
    headers = {"PRIVATE-TOKEN": PRIVATE_TOKEN}
    response = requests.get(url, headers=headers)
    data = response.json()

    data["runners_token"] = "REDACTED"
    data["namespace"]["avatar_url"] = "https://any_url_test.test"
    data["owner"]["avatar_url"] = "https://any_url_test.test"
    data["service_desk_address"] = "https://any_url_test.test"
    data["owner"]["id"] = 123456
    # change to a long time ago to avoid flaky tests
    data["container_expiration_policy"]["next_run_at"] = "2060-03-20T06:26:02.725Z"
    if args.persist:
        persist_contract("project.json", REMOTE, data)
    return data


def get_project_members_api_json():
    url = "https://gitlab.com/api/v4/projects/gitlab-org%2Fgitlab/members"
    headers = {"PRIVATE-TOKEN": PRIVATE_TOKEN}
    # members API is paginated, gather headers to test pagination
    response = requests.get(url, headers=headers)
    # take first two members and fake data
    data = response.json()[:2]
    for i, member in enumerate(data):
        member["avatar_url"] = "https://any_url_test.test" + str(i)
        member["web_url"] = "https://any_url_test.test" + str(i)
        member["id"] = i + 123456
        member["username"] = "test_user_" + str(i)
        member["name"] = "Test User " + str(i)
        member["created_by"]["avatar_url"] = "https://any_url_test.test" + str(i)
        member["created_by"]["web_url"] = "https://any_url_test.test" + str(i)
        member["created_by"]["id"] = i + 123456
        member["created_by"]["username"] = "test_user_" + str(i)
        member["created_by"]["name"] = "Test User " + str(i)
    if args.persist:
        persist_contract("project_members.json", REMOTE, data)
        persist_contract(
            "project_members_response_headers.json", REMOTE, dict(response.headers)
        )
    return response.json()


def merge_request_api():
    mr_base_url = "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests"
    existing_mr_url = f"{mr_base_url}/33"
    headers = {"PRIVATE-TOKEN": PRIVATE_TOKEN}
    response = requests.get(existing_mr_url, headers=headers)
    assert response.status_code == 200
    data = response.json()
    author = data["author"]
    author["id"] = 123456
    author["avatar_url"] = "https://any_url_test.test"
    user = data["head_pipeline"]["user"]
    user["id"] = 123456
    user["avatar_url"] = "https://any_url_test.test"
    if args.persist:
        persist_contract("merge_request.json", REMOTE, data)
    # re-create - response with a 409
    body = {
        "source_branch": "feature",
        "target_branch": "main",
        "title": "New Feature",
    }
    response = requests.post(mr_base_url, headers=headers, data=body)
    assert response.status_code == 409
    data_conflict = response.json()
    if args.persist:
        persist_contract("merge_request_conflict.json", REMOTE, data_conflict)
    return data, data_conflict


def list_pipelines_api():
    # https://docs.gitlab.com/ee/api/pipelines.html
    url = "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/pipelines"
    headers = {"PRIVATE-TOKEN": PRIVATE_TOKEN}
    response = requests.get(url, headers=headers)
    data = response.json()
    if args.persist:
        persist_contract("list_pipelines.json", REMOTE, data)
    return data


class TestAPI:
    def __init__(self, callback, msg, *expected):
        self.callback = callback
        self.msg = msg
        self.expected = expected


if __name__ == "__main__":
    testcases = [
        TestAPI(
            get_project_api_json,
            "project API contract",
            get_contract_json("project.json", REMOTE),
        ),
        TestAPI(
            merge_request_api,
            "merge request API contract",
            get_contract_json("merge_request.json", REMOTE),
            get_contract_json("merge_request_conflict.json", REMOTE),
        ),
        TestAPI(
            list_pipelines_api,
            "list pipelines API contract",
            get_contract_json("list_pipelines.json", REMOTE),
        ),
    ]
    if not validate_responses(testcases):
        exit(1)
    # TODO
    # # get_project_members_api_json()
