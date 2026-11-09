struct Pair {
    bool flag;
    int small;
    unsigned long long big;
    double real;
};
enum Tag : unsigned long long { Empty = 0, Value = 1 };
extern "C" long long inspect_aggregates(Pair value, int (&array)[3], Tag tag) {
    return value.small + array[0] + static_cast<long long>(tag);
}
