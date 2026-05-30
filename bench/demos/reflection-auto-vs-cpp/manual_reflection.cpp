#include <iostream>
#include <string>
#include <unordered_map>
#include <vector>

using RuleFn = long long (*)(long long);

static long long rule_fast_track(long long amount) {
    return amount < 1000 ? 1 : 0;
}

static long long rule_review(long long amount) {
    return amount > 50000 ? 1 : 0;
}

static long long rule_block(long long amount) {
    return amount > 200000 ? 1 : 0;
}

struct Registry {
    std::unordered_map<std::string, RuleFn> rules;

    void reg(const std::string& name, RuleFn fn) {
        rules[name] = fn;
    }

    bool has(const std::string& name) const {
        return rules.find(name) != rules.end();
    }
};

int main() {
    Registry registry;

    // Manual registration burden (common in C++ reflection-like systems):
    // If a developer forgets to register a function, dynamic calls silently miss it.
    registry.reg("rule_fast_track", &rule_fast_track);
    registry.reg("rule_review", &rule_review);
    // Intentionally missing: registry.reg("rule_block", &rule_block);

    std::vector<std::pair<std::string, long long>> call_plan = {
        {"rule_fast_track", 888},
        {"rule_review", 120000},
        {"rule_block", 250000},
    };

    long long sum = 0;
    int missing = 0;
    for (const auto& item : call_plan) {
        const std::string& name = item.first;
        long long arg = item.second;
        auto it = registry.rules.find(name);
        if (it == registry.rules.end()) {
            missing += 1;
            continue;
        }
        sum += it->second(arg);
    }

    std::cout
        << "{"
        << "\"declared_rules\":3,"
        << "\"registered_rules\":" << registry.rules.size() << ","
        << "\"requested_rules\":" << call_plan.size() << ","
        << "\"missing_rules\":" << missing << ","
        << "\"decision_sum\":" << sum
        << "}"
        << std::endl;
    return 0;
}
