import requests
import json
import os
import argparse

PRIVATE_TOKEN = os.environ["GITLAB_TOKEN"]

parser = argparse.ArgumentParser()
parser.add_argument("--persist", action="store_true")
args = parser.parse_args()


def find_expectations(name):
    print("Contract is being used in:")
    os.system("git --no-pager grep -n " + name + " | grep -v contracts")


def persist_contract(name, data):
    with open("contracts/gitlab/{}".format(name), "w") as fh:
        json.dump(data, fh, indent=2)
        fh.write("\n")


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
        persist_contract("project.json", data)
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
        persist_contract("project_members.json", data)
        persist_contract(
            "project_members_response_headers.json", dict(response.headers)
        )
    return response.json()


def create_merge_request_api():
    url = "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests"
    source_branch = "feature"
    target_branch = "main"
    title = "New Feature"
    headers = {"PRIVATE-TOKEN": PRIVATE_TOKEN}
    body = {
        "source_branch": source_branch,
        "target_branch": target_branch,
        "title": title,
    }
    response = requests.post(url, headers=headers, data=body)
    assert response.status_code == 201
    data = response.json()
    if args.persist:
        persist_contract("merge_request.json", data)
    # re-create - response with a 409
    response = requests.post(url, headers=headers, data=body)
    assert response.status_code == 409
    data_conflict = response.json()
    if args.persist:
        persist_contract("merge_request_conflict.json", data_conflict)
    return data, data_conflict


def get_contract_json(name):
    with open("contracts/gitlab/{}".format(name)) as fh:
        return json.load(fh)


def _verify_all_keys_exist(expected, actual):
    for key in expected:
        if key not in actual:
            print("Expected JSON key [{}] not found in upstream".format(key))
            return False
        if type(expected[key]) == dict:
            # API responses checked are not more than one level deep
            if not _verify_all_keys_exist(expected[key], actual[key]):
                return False
    return True


def _verify_types_of_values(expected, actual):
    for key in expected:
        if type(expected[key]) != type(actual[key]):
            print(
                "Type mismatch for key [{}]: expected [{}] but got [{}]".format(
                    key, type(expected[key]), type(actual[key])
                )
            )
            return False
        if type(expected[key]) == dict:
            # API responses checked are not more than one level deep
            if not _verify_types_of_values(expected[key], actual[key]):
                return False
    return True


def verify_all(expected, actual):
    if not _verify_all_keys_exist(expected, actual):
        return False
    if not _verify_types_of_values(expected, actual):
        return False
    return True


class TestAPI:
    def __init__(self, callback, msg, *expected):
        self.callback = callback
        self.msg = msg
        self.expected = expected


def validate_responses(testcases):
    for testcase in testcases:
        actual = testcase.callback()
        print("{}... ".format(testcase.msg), end="")
        verifications = []
        if type(actual) == tuple:
            verifications = zip(testcase.expected, actual)
        else:
            verifications = zip(testcase.expected, [actual])
        for expected, actual in verifications:
            if not verify_all(expected, actual):
                return False
        print("OK")
    return True


if __name__ == "__main__":
    testcases = [
        TestAPI(
            get_project_api_json,
            "project API contract",
            get_contract_json("project.json"),
            # TODO: teardown callback close merge request
        ),
        TestAPI(
            create_merge_request_api,
            "merge request API contract",
            get_contract_json("merge_request.json"),
            get_contract_json("merge_request_conflict.json"),
        ),
    ]
    if not validate_responses(testcases):
        exit(1)
    # TODO
    # # get_project_members_api_json()
