#include <cstdlib>
#include <cstring>
#include <dlfcn.h>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <string>

#include "EmQuantAPI.h"

using SetServerListDir = void (*)(const char *);
using Start = EQErr (*)(EQLOGININFO *, const char *, logcallback);
using Stop = EQErr (*)();
using Snapshot = EQErr (*)(const char *, const char *, const char *, EQDATA *&);
using ReleaseData = EQErr (*)(void *);

static int discard_log(const char *) { return 0; }

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

int main(int argc, char **argv) {
    if (argc < 3 || argc > 4) {
        std::cerr << "usage: emquant-snapshot CODE[,CODE] INDICATOR[,INDICATOR] [OPTIONS]\n";
        return 2;
    }
    const char *library = std::getenv("MAGIC_EMQUANT_LIB");
    const char *server_dir = std::getenv("MAGIC_EMQUANT_SERVER_LIST");
    const char *username = std::getenv("MAGIC_EMQUANT_USERNAME");
    const char *password = std::getenv("MAGIC_EMQUANT_PASSWORD");
    if (!library || !server_dir) {
        std::cerr << "missing MAGIC_EMQUANT_LIB or MAGIC_EMQUANT_SERVER_LIST\n";
        return 2;
    }
    const bool has_username = username && username[0] != '\0';
    const bool has_password = password && password[0] != '\0';
    if (has_username != has_password) {
        std::cerr << "MAGIC_EMQUANT_USERNAME and MAGIC_EMQUANT_PASSWORD must be set together\n";
        return 2;
    }
    void *handle = dlopen(library, RTLD_NOW | RTLD_LOCAL);
    if (!handle) {
        std::cerr << "unable to load EMQuant library: " << dlerror() << '\n';
        return 3;
    }
    const auto set_server_dir = symbol<SetServerListDir>(handle, "setserverlistdir");
    const auto start = symbol<Start>(handle, "start");
    const auto stop = symbol<Stop>(handle, "stop");
    const auto snapshot = symbol<Snapshot>(handle, "csqsnapshot");
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
        std::cerr << "EMQuant login failed with code " << login_error << '\n';
        dlclose(handle);
        return 4;
    }
    EQDATA *data = nullptr;
    const EQErr query_error = snapshot(argv[1], argv[2], argc == 4 ? argv[3] : "", data);
    if (query_error != EQERR_SUCCESS || !data) {
        std::cerr << "EMQuant snapshot failed with code " << query_error << '\n';
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
