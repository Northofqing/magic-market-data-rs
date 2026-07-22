#include <cstdlib>
#include <cstring>
#include <dlfcn.h>
#include <iomanip>
#include <iostream>
#include <limits.h>
#include <sstream>
#include <string>
#include <unistd.h>

#include "EmQuantAPI.h"

using SetServerListDir = void (*)(const char *);
using Start = EQErr (*)(EQLOGININFO *, const char *, logcallback);
using Stop = EQErr (*)();
using Snapshot = EQErr (*)(const char *, const char *, const char *, EQDATA *&);
using History = EQErr (*)(const char *, const char *, const char *, const char *, const char *, EQDATA *&);
using ReleaseData = EQErr (*)(void *);

static int discard_log(const char *) { return 0; }

static std::string executable_dir(const char *argv0) {
    char resolved[PATH_MAX];
    const char *path = argv0;
    if (argv0 && realpath(argv0, resolved)) path = resolved;
    std::string value = path ? path : "";
    const std::string::size_type slash = value.find_last_of('/');
    if (slash != std::string::npos) return value.substr(0, slash);
    char current[PATH_MAX];
    return getcwd(current, sizeof(current)) ? current : ".";
}

static std::string json_string(const char *value) {
    std::ostringstream out;
    out << '"';
    for (const unsigned char ch : std::string(value ? value : "")) {
        switch (ch) {
        case '"': out << "\\\""; break;
        case '\\': out << "\\\\"; break;
        case '\n': out << "\\n"; break;
        case '\r': out << "\\r"; break;
        case '\t': out << "\\t"; break;
        default:
            if (ch < 0x20) out << "\\u" << std::hex << std::setw(4) << std::setfill('0') << int(ch);
            else out << ch;
        }
    }
    out << '"';
    return out.str();
}

static std::string value_json(const EQVARIENT *value) {
    if (!value || value->vtype == eVT_null) return "null";
    std::ostringstream out;
    out << std::setprecision(17);
    switch (value->vtype) {
    case eVT_char: return json_string(std::string(1, value->unionValues.charValue).c_str());
    case eVT_bool: return value->unionValues.boolValue ? "true" : "false";
    case eVT_int: out << value->unionValues.intValue; break;
    case eVT_uInt: out << value->unionValues.uIntValue; break;
    case eVT_int64: out << value->unionValues.int64Value; break;
    case eVT_uInt64: out << value->unionValues.uInt64Value; break;
    case eVT_float: out << value->unionValues.floatValue; break;
    case eVT_double: out << value->unionValues.doubleValue; break;
    case eVT_short: out << value->unionValues.shortValue; break;
    case eVT_ushort: out << value->unionValues.uShortValue; break;
    case eVT_asciiString:
    case eVT_unicodeString: return json_string(value->eqchar.pChar);
    default: return "null";
    }
    return out.str();
}

template <typename T> static T symbol(void *handle, const char *name) {
    dlerror();
    void *raw = dlsym(handle, name);
    const char *error = dlerror();
    if (error) {
        std::cerr << "missing EMQuant symbol " << name << ": " << error << '\n';
        std::exit(3);
    }
    return reinterpret_cast<T>(raw);
}

static const char *login_error_hint(EQErr error) {
    switch (error) {
    case EQERR_NO_ACCESS:
        return "EQERR_NO_ACCESS: the account has no EMQuant API entitlement; enable API access in QuantAPI/Choice or contact the account manager";
    case EQERR_ACCESS_EXPIRE:
        return "EQERR_ACCESS_EXPIRE: the account's EMQuant API entitlement has expired";
    case EQERR_NO_LV2_ACCESS:
        return "EQERR_NO_LV2_ACCESS: the account has no EMQuant Level-2 entitlement";
    case EQERR_LV2_ACCESS_EXPIRE:
        return "EQERR_LV2_ACCESS_EXPIRE: the account's EMQuant Level-2 entitlement has expired";
    case EQERR_LOGIN_COUNT_LIMIT:
        return "EQERR_LOGIN_COUNT_LIMIT: the account has reached its concurrent API login limit";
    case EQERR_DIFFRENT_DEVICE:
        return "EQERR_DIFFRENT_DEVICE: userInfo was activated on a different device; activate again on this machine";
    case EQERR_USERINFO_EXPIRED:
        return "EQERR_USERINFO_EXPIRED: userInfo has expired; run the activator again";
    default:
        return nullptr;
    }
}

int main(int argc, char **argv) {
    const bool history_mode = argc > 1 && std::strcmp(argv[1], "--history") == 0;
    const bool section_mode = argc > 1 && std::strcmp(argv[1], "--section") == 0;
    if ((!history_mode && !section_mode && (argc < 3 || argc > 4))
        || (history_mode && argc != 8) || (section_mode && argc != 6)) {
        std::cerr << "usage: emquant-snapshot CODE[,CODE] INDICATOR[,INDICATOR] [OPTIONS]\n"
                  << "   or: emquant-snapshot --history csd|chmc CODE INDICATOR[,INDICATOR] START END OPTIONS\n"
                  << "   or: emquant-snapshot --section css CODE[,CODE] INDICATOR[,INDICATOR] OPTIONS\n";
        return 2;
    }
    if (history_mode && std::strcmp(argv[2], "csd") != 0 && std::strcmp(argv[2], "chmc") != 0) {
        std::cerr << "unsupported EMQuant history method; expected csd or chmc\n";
        return 2;
    }
    if (section_mode && std::strcmp(argv[2], "css") != 0) {
        std::cerr << "unsupported EMQuant section method; expected css\n";
        return 2;
    }
    const std::string runtime_dir = executable_dir(argv[0]) + "/runtime";
    const std::string default_library = runtime_dir + "/libEMQuantAPIx64.dylib";
    const char *library_env = std::getenv("MAGIC_EMQUANT_LIB");
    const char *server_dir_env = std::getenv("MAGIC_EMQUANT_SERVER_LIST");
    const char *library = library_env && library_env[0] != '\0'
        ? library_env : default_library.c_str();
    const char *server_dir = server_dir_env && server_dir_env[0] != '\0'
        ? server_dir_env : runtime_dir.c_str();
    const char *username = std::getenv("MAGIC_EMQUANT_USERNAME");
    const char *password = std::getenv("MAGIC_EMQUANT_PASSWORD");
    const bool has_username = username && username[0] != '\0';
    const bool has_password = password && password[0] != '\0';
    if (has_username != has_password) {
        std::cerr << "MAGIC_EMQUANT_USERNAME and MAGIC_EMQUANT_PASSWORD must be set together\n";
        return 2;
    }
    void *handle = dlopen(library, RTLD_NOW | RTLD_LOCAL);
    if (!handle) {
        std::cerr << "unable to load EMQuant library " << library << ": " << dlerror() << '\n';
        return 3;
    }
    const auto set_server_dir = symbol<SetServerListDir>(handle, "setserverlistdir");
    const auto start = symbol<Start>(handle, "start");
    const auto stop = symbol<Stop>(handle, "stop");
    const auto release_data = symbol<ReleaseData>(handle, "releasedata");
    set_server_dir(server_dir);
    EQLOGININFO login{};
    EQLOGININFO *login_pointer = nullptr;
    if (has_username && has_password) {
        std::strncpy(login.userName, username, sizeof(login.userName) - 1);
        std::strncpy(login.password, password, sizeof(login.password) - 1);
        login_pointer = &login;
    }
    const EQErr login_error = start(login_pointer, "TestLatency=0,ForceLogin=0,LogLevel=0", discard_log);
    if (login_error != EQERR_SUCCESS) {
        std::cerr << "EMQuant login failed with code " << login_error;
        if (login_error == EQERR_NEED_ACTIVATE) {
            std::cerr << " (EQERR_NEED_ACTIVATE: run " << runtime_dir
                      << "/loginactivator_mac and complete API activation)";
        } else if (const char *hint = login_error_hint(login_error)) {
            std::cerr << " (" << hint << ')';
        }
        std::cerr << '\n';
        dlclose(handle);
        return 4;
    }
    EQDATA *data = nullptr;
    EQErr query_error = EQERR_SUCCESS;
    if (history_mode) {
        const auto history = symbol<History>(handle, argv[2]);
        query_error = history(argv[3], argv[4], argv[5], argv[6], argv[7], data);
    } else if (section_mode) {
        const auto section = symbol<Snapshot>(handle, "css");
        query_error = section(argv[3], argv[4], argv[5], data);
    } else {
        const auto snapshot = symbol<Snapshot>(handle, "csqsnapshot");
        query_error = snapshot(argv[1], argv[2], argc == 4 ? argv[3] : "", data);
    }
    if (query_error != EQERR_SUCCESS || !data) {
        std::cerr << "EMQuant query failed with code " << query_error << '\n';
        if (data) release_data(data);
        stop();
        dlclose(handle);
        return 5;
    }
    std::cout << "{\"records\":[";
    bool first = true;
    for (unsigned int date = 0; date < data->dateArray.nSize; ++date) {
        for (unsigned int code = 0; code < data->codeArray.nSize; ++code) {
            if (!first) std::cout << ',';
            first = false;
            std::cout << "{\"date\":" << json_string(data->dateArray.pChArray[date].pChar)
                      << ",\"code\":" << json_string(data->codeArray.pChArray[code].pChar)
                      << ",\"values\":{";
            for (unsigned int indicator = 0; indicator < data->indicatorArray.nSize; ++indicator) {
                if (indicator) std::cout << ',';
                std::cout << json_string(data->indicatorArray.pChArray[indicator].pChar) << ':'
                          << value_json((*data)(code, indicator, date));
            }
            std::cout << "}}";
        }
    }
    std::cout << "]}\n";
    release_data(data);
    stop();
    dlclose(handle);
    return 0;
}
