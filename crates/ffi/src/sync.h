#pragma once

#include <atomic>
#include <condition_variable>
#include <memory>
#include <mutex>
#include <optional>
#include <queue>
// #include <shared_mutex>
#include <utility>

namespace task {
namespace sync {
// namespace watch {
//
// namespace state {
//
// static constexpr size_t CLOSED_BIT = 1;
// static constexpr size_t STEP_SIZE = 2;
//
// struct snapshot {
// private:
//   size_t m_value;
//
// public:
//   snapshot(size_t value) : m_value(value) {}
//
// public:
//   size_t version() const { return m_value & !CLOSED_BIT; }
//   bool is_closed() const { return (m_value & CLOSED_BIT) == CLOSED_BIT; }
// };
//
// struct atomic_state {
// private:
//   static constexpr size_t INITIAL = 0;
//   mutable std::atomic_size_t m_state = INITIAL;
//
// public:
//   atomic_state() : m_state(INITIAL) {}
//
// public:
//   snapshot load() const {
//     return snapshot(m_state.load(std::memory_order_acquire));
//   }
//
//   void increment_version_while_locked() const {
//     // Use `Release` ordering to ensure that the shared value
//     // has been written before updating the version. The shared
//     // value is still protected by an exclusive lock during this
//     // method.
//     m_state.fetch_add(STEP_SIZE, std::memory_order_release);
//   }
//
//   void set_closed() const {
//     m_state.fetch_or(CLOSED_BIT, std::memory_order_release);
//   }
// };
// }
//
// template <typename T> struct shared {
//
// public:
//   std::shared_mutex m_value;
//   state::atomic_state m_sate;
//   std::atomic_size_t m_ref_count_rx;
//   std::atomic_size_t m_ref_count_tx;
//   std::condition_variable_any m_cond;
//
// public:
//   shared() {}
//
// public:
// private:
// };
//
// template <typename T> class ref {
// private:
//   std::shared_lock<std::shared_mutex> m_guard;
//   std::shared_ptr<T> m_value;
//   bool m_has_changed;
//
// public:
//   ref(std::shared_lock<std::shared_mutex> &&guard, std::shared_ptr<T> value,
//       bool has_changed)
//       : m_guard(std::move(guard)), m_value(value), m_has_changed(has_changed)
//       {}
//
// public:
//   inline bool has_changed() { return m_has_changed; };
// };
//
// template <typename T> class sender {
// private:
//   std::shared_ptr<shared<T>> m_shared;
//
// public:
//   sender(const sender &);
//   sender(sender &&) noexcept;
//   sender(decltype(m_shared));
//   ~sender();
//
// public:
//   bool send(T && = {});
// };
//
// template <typename T> class receiver {
// private:
//   std::shared_ptr<shared<T>> m_shared;
//   size_t m_version;
//
// public:
//   receiver(receiver &&) noexcept;
//   receiver(const receiver &) = default;
//   receiver(decltype(m_shared));
//   ~receiver();
//
// public:
//   std::optional<T> receive();
// };
//
// template <typename T> std::pair<sender<T>, receiver<T>> channel(T &&);
//
// template <typename T> struct channel_t {
//   using sender_t = sender<T>;
//   using receiver_t = receiver<T>;
// };
//
// template <typename> struct channel_from;
// template <typename T>
// struct channel_from<channel_t<T>> : public std::pair<sender<T>, receiver<T>>
// {
//   channel_from(T &&);
// };
//
// template <typename T> sender<T>::sender(const sender &other) = default;
// template <typename T> sender<T>::sender(sender &&other) noexcept = default;
// template <typename T>
// sender<T>::sender(decltype(m_mutex) mutex, decltype(m_value) value,
//                   decltype(m_cond) cond, decltype(m_closed) closed)
//     : m_mutex(std::move(mutex)), m_value(std::move(value)),
//       m_cond(std::move(cond)), m_closed(std::move(closed)) {}
//
// template <typename T> sender<T>::~sender() {
//   m_closed->store(true);
//   m_cond->notify_one();
// }
//
// template <typename T> bool sender<T>::send(T &&value) {
//   if (m_closed->load()) {
//     return false;
//   }
//   {
//     std::lock_guard guard(*m_mutex);
//     m_value = std::forward<T>(value);
//   }
//   m_cond->notify_all();
// }
//
// template <typename T>
// receiver<T>::receiver(receiver &&other) noexcept = default;
// template <typename T>
// receiver<T>::receiver(decltype(m_mutex) mutex, decltype(m_value) value,
//                       decltype(m_cond) cond, decltype(m_closed) closed)
//     : m_mutex(std::move(mutex)), m_value(std::move(value)),
//       m_cond(std::move(cond)), m_closed(std::move(closed)) {}
//
// template <typename T> receiver<T>::~receiver() { m_closed->store(true); }
//
// template <typename T> std::optional<T> receiver<T>::receive() {
//
//   if (m_closed->load()) {
//     return std::nullopt;
//   }
//
//   std::unique_lock lock(*m_mutex);
//   m_cond->wait(lock);
//
//   if (m_closed->load()) {
//     return std::nullopt;
//   }
//
//   T value = *m_value;
//   return std::move(value);
// }
//
// template <typename T> std::pair<sender<T>, receiver<T>> channel(T &&initial)
// {
//   auto mutex = std::make_shared<std::mutex>();
//   auto value = std::make_shared<T>(initial);
//   auto cond = std::make_shared<std::condition_variable>();
//   auto closed = std::make_shared<std::atomic_bool>(false);
//
//   return {sender<T>{mutex, value, cond, closed},
//           receiver<T>{mutex, value, cond, closed}};
// }
//
// template <typename T>
// channel_from<channel_t<T>>::channel_from(T &&initial)
//     : std::pair<sender<T>, receiver<T>>(channel<T>(initial)) {}
//
// } // namespace watch

namespace mpsc {

template <typename T> class sender {
private:
  std::shared_ptr<std::mutex> m_mutex;
  std::shared_ptr<std::queue<T>> m_queue;
  std::shared_ptr<std::condition_variable> m_cond;
  std::shared_ptr<std::atomic_bool> m_closed;

public:
  sender(const sender &);
  sender(sender &&) noexcept;
  sender(decltype(m_mutex), decltype(m_queue), decltype(m_cond),
         decltype(m_closed));
  ~sender();

public:
  bool send(T && = {});
};

template <typename T> class receiver {
private:
  std::shared_ptr<std::mutex> m_mutex;
  std::shared_ptr<std::queue<T>> m_queue;
  std::shared_ptr<std::condition_variable> m_cond;
  std::shared_ptr<std::atomic_bool> m_closed;

public:
  receiver(receiver &&) noexcept;
  receiver(const receiver &) = delete; // single consumer, no copy
  receiver(decltype(m_mutex), decltype(m_queue), decltype(m_cond),
           decltype(m_closed));
  ~receiver();

public:
  std::optional<T> receive();
};

template <typename T> std::pair<sender<T>, receiver<T>> channel(T &&);

template <typename T> struct channel_t {
  using sender_t = sender<T>;
  using receiver_t = receiver<T>;
};

template <typename> struct channel_from;
template <typename T>
struct channel_from<channel_t<T>> : public std::pair<sender<T>, receiver<T>> {
  channel_from(T &&);
};

template <typename T> sender<T>::sender(const sender &other) = default;
template <typename T> sender<T>::sender(sender &&other) noexcept = default;
template <typename T>
sender<T>::sender(decltype(m_mutex) mutex, decltype(m_queue) value,
                  decltype(m_cond) cond, decltype(m_closed) closed)
    : m_mutex(std::move(mutex)), m_queue(std::move(value)),
      m_cond(std::move(cond)), m_closed(std::move(closed)) {}

template <typename T> sender<T>::~sender() {
  m_closed->store(true);
  m_cond->notify_one();
}

template <typename T> bool sender<T>::send(T &&value) {
  if (m_closed->load()) {
    return false;
  }
  {
    std::lock_guard guard(*m_mutex);
    m_queue = std::forward<T>(value);
  }
  m_cond->notify_one();
}

template <typename T>
receiver<T>::receiver(receiver &&other) noexcept = default;
template <typename T>
receiver<T>::receiver(decltype(m_mutex) mutex, decltype(m_queue) value,
                      decltype(m_cond) cond, decltype(m_closed) closed)
    : m_mutex(std::move(mutex)), m_queue(std::move(value)),
      m_cond(std::move(cond)), m_closed(std::move(closed)) {}

template <typename T> receiver<T>::~receiver() { m_closed->store(true); }

template <typename T> std::optional<T> receiver<T>::receive() {

  if (m_closed->load()) {
    return std::nullopt;
  }

  std::unique_lock lock(*m_mutex);
  m_cond->wait(lock);

  if (m_closed->load()) {
    return std::nullopt;
  }

  T value = *m_queue;
  return std::move(value);
}

template <typename T> std::pair<sender<T>, receiver<T>> channel(T &&initial) {
  auto mutex = std::make_shared<std::mutex>();
  auto value = std::make_shared<T>(initial);
  auto cond = std::make_shared<std::condition_variable>();
  auto closed = std::make_shared<std::atomic_bool>(false);

  return {sender<T>{mutex, value, cond, closed},
          receiver<T>{mutex, value, cond, closed}};
}

template <typename T>
channel_from<channel_t<T>>::channel_from(T &&initial)
    : std::pair<sender<T>, receiver<T>>(channel<T>(initial)) {}

} // namespace mpsc
} // namespace sync
} // namespace task
