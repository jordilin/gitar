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
