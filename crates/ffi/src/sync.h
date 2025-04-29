#pragma once

#include <atomic>
#include <condition_variable>
#include <memory>
#include <mutex>
#include <optional>
#include <utility>

namespace task {
namespace sync {
namespace watch {

template <typename T> class sender {
private:
  std::shared_ptr<std::mutex> m_mutex;
  std::shared_ptr<T> m_value;
  std::shared_ptr<std::condition_variable> m_cond;
  std::shared_ptr<std::atomic_bool> m_closed;

public:
  sender(const sender &);
  sender(sender &&) noexcept;
  sender(decltype(m_mutex), decltype(m_value), decltype(m_cond),
         decltype(m_closed));
  ~sender();

public:
  bool send(T && = {});
};

template <typename T> class receiver {
private:
  std::shared_ptr<std::mutex> m_mutex;
  std::shared_ptr<T> m_value;
  std::shared_ptr<std::condition_variable> m_cond;
  std::shared_ptr<std::atomic_bool> m_closed;

public:
  receiver(const receiver &);
  receiver(receiver &&) noexcept;
  receiver(decltype(m_mutex), decltype(m_value), decltype(m_cond),
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
sender<T>::sender(decltype(m_mutex) mutex, decltype(m_value) value,
                  decltype(m_cond) cond, decltype(m_closed) closed)
    : m_mutex(std::move(mutex)), m_value(std::move(value)),
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
    m_value = std::forward<T>(value);
  }
  m_cond->notify_one();
}

template <typename T> receiver<T>::receiver(const receiver &other) = default;
template <typename T>
receiver<T>::receiver(receiver &&other) noexcept = default;
template <typename T>
receiver<T>::receiver(decltype(m_mutex) mutex, decltype(m_value) value,
                      decltype(m_cond) cond, decltype(m_closed) closed)
    : m_mutex(std::move(mutex)), m_value(std::move(value)),
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

  T value = *m_value;
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

} // namespace watch
} // namespace sync
} // namespace task
